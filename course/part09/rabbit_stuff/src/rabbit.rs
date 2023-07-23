use std::{fmt::Debug, sync::Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_channel::Receiver;
use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::StreamExt;
use lapin::{
    options::{
        BasicAckOptions,
        BasicConsumeOptions,
        BasicNackOptions,
        BasicPublishOptions,
        ExchangeDeclareOptions,
        QueueBindOptions,
        QueueDeclareOptions,
    },
    protocol::constants::REPLY_SUCCESS,
    publisher_confirm::Confirmation,
    types::{AMQPValue::LongString, FieldTable},
    BasicProperties,
    Channel,
    Connection,
    ConnectionProperties,
    Consumer,
    ExchangeKind,
};
use lapin::message::Delivery;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument, Instrument, info, info_span};

pub const QUEUE: &str = "queue-joseph";
pub const EXCHANGE: &str = "exchange-joseph";
const ROUTING: &str = "";
const CONSUMER_TAG: &str = "joseph-consumer";
pub const MESSAGE_TYPE: &str = "msg-joseph";
pub const MESSAGE_TYPE_2: &str = "msg-joseph-2";

pub struct Rabbit {
    conn: Connection,
    chan: Channel,
}

fn ft_default() -> FieldTable {
    let mut ft = FieldTable::default();
    ft.insert("x-match".into(), LongString("all".into()));
    ft
}

impl Rabbit {
    pub async fn new(address: &str) -> Result<Rabbit, lapin::Error> {
        let connection_properties = ConnectionProperties::default();
        let conn = Connection::connect(address, connection_properties).await?;

        let chan = conn.create_channel().await?;

        Ok(Rabbit { conn, chan })
    }

    // ensure exchange + queue exist and bind them together
    pub async fn setup(&self) -> Result<(), lapin::Error> {
        self.chan
            .exchange_declare(
                EXCHANGE,
                ExchangeKind::Headers,
                ExchangeDeclareOptions::default(),
                ft_default(),
            )
            .await?;

        self.chan
            .queue_declare(
                QUEUE,
                QueueDeclareOptions::default(),
                ft_default(),
            )
            .await?;

        self.chan
            .queue_bind(
                QUEUE,
                EXCHANGE,
                ROUTING,
                QueueBindOptions::default(),
                ft_default(),
            )
            .await?;

        Ok(())
    }

    pub async fn close(&self) -> Result<(), lapin::Error> {
        let err1 = self.chan.close(REPLY_SUCCESS, "thank you!").await;
        let err2 = self.conn.close(REPLY_SUCCESS, "thank you!").await;
        err1?;
        err2
    }

    // publishes a message to the provided exchange with a json serialized body
    pub async fn publish_json<S: Serialize>(
        &self,
        exchange: &str,
        message_type: &str,
        body: S,
    ) -> Result<Confirmation, PublishError> {
        let body = serde_json::to_string(&body)?;
        let mut headers = ft_default();
        headers.insert("content-type".into(), LongString("application/json".into()));
        headers.insert("message_type".into(), LongString(message_type.into()));
        self.chan
            .basic_publish(
                exchange,
                ROUTING,
                BasicPublishOptions::default(),
                body.as_bytes(),
                BasicProperties::default().with_headers(headers),
            )
            .await?
            .await
            .map_err(Into::into)
    }

    // consumes messages from a queue and the delegator is responsible for
    // ensuring thew messages get consumed. in the provided implementations
    // this means by a RabbitConsumer if the message-type header matches
    #[instrument(skip(self, rabbit_delegator, kill_signal))]
    pub async fn consume<D: RabbitDelegator>(
        &self,
        queue: &str,
        rabbit_delegator: D,
        kill_signal: CancellationToken,
    ) -> Result<JoinHandle<()>, lapin::Error> {
        let consumer = self
            .chan
            .basic_consume(
                queue,
                CONSUMER_TAG,
                BasicConsumeOptions::default(),
                ft_default(),
            )
            .await?;

        Ok(tokio::spawn(
            run_consumer(rabbit_delegator, consumer, self.chan.clone(), kill_signal).in_current_span()
        ))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PublishError {
    #[error("failed to serialize struct: {0}")]
    SerializeError(#[from] serde_json::error::Error),
    #[error("rabbit operation failed: {0}")]
    RabbitError(#[from] lapin::Error),
}

async fn run_consumer<D: RabbitDelegator>(
    delegator: D,
    mut consumer: Consumer,
    channel: Channel,
    kill_signal: CancellationToken,
) {
    let (sender, receiver) = async_channel::unbounded();

    // put delegator in an arc as we need to share it between the workers
    let delegator = Arc::new(delegator);

    // creates 10 workers for the queue & passes messages to them over a channel
    // there is a builtin lapin::Consumer::set_delegate, but i wanted to limit
    // the parallelism
    let handles = (0..10)
        .map(|i| {
            let span = info_span!("worker", "num" = i);
            let channel = channel.clone();
            let delegator = Arc::clone(&delegator);
            let receiver = receiver.clone();
            tokio::spawn(
                worker(channel, receiver, delegator).instrument(span),
            )
        })
        .collect::<Vec<_>>();

    loop {
        let delivery: Option<Result<Delivery, lapin::Error>> = select! {
            delivery = consumer.next() => delivery,
            _ = kill_signal.cancelled() => break,
        };

        // None if consumer cancelled
        let Some(delivery) = delivery else {
            error!("consumer was unexpectedly cancelled");
            break;
        };

        let delivery = match delivery {
            Ok(delivery) => delivery,
            Err(err) => {
                error!("error on delivery?: {}", err);
                continue;
            }
        };

        if let Err(err) = sender
            .send(delivery)
            .await
        {
            error!("rabbit consumer failed to send message: {err}");
            break;
        }
    }

    // close channel so workers shut down after
    // finishing processing their current message
    sender.close();

    for handle in handles {
        if let Err(err) = handle.await {
            error!("worker handle failure: {err}")
        }
    }
}


// a worker is responsible for processing a lapin::message::Delivery
// via the delegator
async fn worker<D: RabbitDelegator>(
    channel: Channel,
    mut receiver: Receiver<Delivery>,
    delegator: Arc<D>
) {
    // consumes from channel whilst it's not closed
    while let Some(delivery) = receiver.next().await {
        let Some(header) = delivery
            .properties
            .headers()
            .as_ref()
            .unwrap()
            .inner()
            .get("message_type")
            .and_then(|message_type| message_type.as_long_string())
            .map(|message_type| message_type.to_string())
        else {
            continue;
        };

        let delivery_tag = delivery.delivery_tag;
        let contents = delivery.data;

        let span = info_span!("processing message", header);

        // async{}.instrument(...).await is used as we cannot use
        // let _entered = span.enter() in async. .instrument allows us
        // to enter & exit from the span's scope when the future is polled, whereas
        // span.enter() would be entered for the entire time the future exists and not
        // just when it's running
        async {
            let delegate_result = delegator.delegate(&header, contents).await;

            // on success we ack, on failure we rack & requeue if the error allows for
            //it (due to reasons such as transient failures etc)
            match delegate_result {
                Ok(_) => {
                    if let Err(err) = channel
                        .basic_ack(delivery_tag, BasicAckOptions::default())
                        .await
                    {
                        error!("failed to ack msg: {}", err);
                    }
                }
                Err(err) => {
                    let requeue = err.should_requeue().into();
                    error!("failed to delegate message: {err} - requeue = {requeue}");
                    if let Err(err) = channel
                        .basic_nack(
                            delivery_tag,
                            BasicNackOptions {
                                requeue,
                                ..Default::default()
                            },
                        )
                        .await
                    {
                        error!("failed to nack msg: {}", err);
                    }
                }
            }
        }.instrument(span).await;
    }

    info!("shutting down worker!")
}

// A trait for rabbit consumer/delegator errors that decides if a message should be
// requeued or not. defaults to not requeueing
pub trait ShouldRequeue {
    fn should_requeue(&self) -> Requeue {
        Requeue::No
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Requeue {
    Yes,
    No,
}

impl From<Requeue> for bool {
    fn from(value: Requeue) -> Self {
        match value {
            Requeue::Yes => true,
            Requeue::No => false,
        }
    }
}

// A trait that represents a consumer of a specific rabbit message_type header
#[async_trait]
pub trait RabbitConsumer: Sync + Send + 'static {
    const MESSAGE_TYPE_HEADER: &'static str;

    type Message<'a>: Deserialize<'a> + Send;
    type ConsumerError: RequeueableError + From<serde_json::Error>;

    fn parse_msg<'a>(&self, contents: &'a [u8]) -> Result<Self::Message<'a>, Self::ConsumerError> {
        serde_json::from_slice(contents).map_err(Into::into)
    }

    async fn process(&self, msg: Self::Message<'_>) -> Result<(), Self::ConsumerError>;

    async fn try_process(&self, contents: Vec<u8>) -> Result<(), Box<dyn RequeueableError>> {
        self.try_process_inner(contents).await.map_err(|err| {
            error!("failed to process message: {}", err);
            Box::new(err) as Box<dyn RequeueableError>
        })
    }

    async fn try_process_inner(&self, contents: Vec<u8>) -> Result<(), Self::ConsumerError> {
        let message = self.parse_msg(&contents)?;
        self.process(message).await
    }
}

// RabbitDelegator represents a thing which 'delegates' an incoming message from a rabbit queue
// to one of multiple consumers. the macro impls below create delegators of
// tuples of rabbit consumers & simply checks their headers match
// before passing it to the appropriate consumer
pub trait RabbitDelegator: Send + Sync + 'static {
    fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut;
}

#[pin_project(project=DelegateFutProj)]
pub enum DelegateFut<'a> {
    // we are using `async_trait` on consumers so this is unavoidable
    ConsumerFut(#[pin] BoxFuture<'a, Result<(), Box<dyn RequeueableError>>>),
    NoHeaderMatch
}

#[derive(thiserror::Error, Debug)]
#[error("delegator was unable to match the message-type header")]
struct NoHeaderMatch;

// if the headers didnt match now, they never will so default ShouldRequeue::No is fine
impl ShouldRequeue for NoHeaderMatch {}

// a trait for all consumer errors, needed for the Box<dyn>
// as that can only contain 1 trait + any auto traits
pub trait RequeueableError: std::error::Error + ShouldRequeue + Send {}

// automatically implement for all appropriate types, no need to do it manually!
impl<T> RequeueableError for T where T: std::error::Error + ShouldRequeue + Send {}

// DelegateFut returns a type erased error object, similar to the error
// interface in go. we do this because we could have any number of errors
// and the cost of boxing probably isn't very high considering that failures
// are not the expected case
impl<'a> Future for DelegateFut<'a> {
    type Output = Result<(), Box<dyn RequeueableError>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            DelegateFutProj::ConsumerFut(f) => f.poll(cx),
            DelegateFutProj::NoHeaderMatch => Poll::Ready(Err(Box::new(NoHeaderMatch)))
        }
    }
}

macro_rules! delegator_tuple {
    ( $ty:tt ) => {
        impl< $ty > RabbitDelegator for $ty
        where
            $ty: RabbitConsumer
        {
            fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut {
                if $ty::MESSAGE_TYPE_HEADER == header {
                    return DelegateFut::ConsumerFut(self.try_process(contents));
                }
                DelegateFut::NoHeaderMatch
            }
        }

        #[async_trait]
        impl< $ty > RabbitDelegator for ($ty,)
        where
            $ty: RabbitConsumer
        {
            fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut {
                let (casey::lower!($ty),) = self;
                if $ty::MESSAGE_TYPE_HEADER == header {
                    return DelegateFut::ConsumerFut(casey::lower!($ty).try_process(contents));
                }
                DelegateFut::NoHeaderMatch
            }
        }
    };
    ( $($ty:tt),* ) => {
        #[async_trait]
        impl< $($ty),* > RabbitDelegator for (
            $($ty),*
        )
        where
            $($ty: RabbitConsumer),*
        {
            fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut {
                let ($(casey::lower!($ty)),*) = self;
                $(
                if $ty::MESSAGE_TYPE_HEADER == header {
                    return DelegateFut::ConsumerFut(casey::lower!($ty).try_process(contents));
                }
                )*
                DelegateFut::NoHeaderMatch
            }
        }
    }
}

// delegator implementations for a single RabbitConsumer and
// tuples of size 1 to 10 of RabbitConsumers
delegator_tuple!(A);
delegator_tuple!(A, B);
delegator_tuple!(A, B, C);
delegator_tuple!(A, B, C, D);
delegator_tuple!(A, B, C, D, E);
delegator_tuple!(A, B, C, D, E, F);
delegator_tuple!(A, B, C, D, E, F, G);
delegator_tuple!(A, B, C, D, E, F, G, H);
delegator_tuple!(A, B, C, D, E, F, G, H, I);
delegator_tuple!(A, B, C, D, E, F, G, H, I, J);
