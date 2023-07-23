use std::{fmt::Debug, sync::Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_channel::{Receiver, Sender};
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
use thiserror::Error;
use tokio::select;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument, span::Span, Instrument, info, info_span};

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

    pub async fn declare_exchange(&self, exchange: &str) -> Result<(), lapin::Error> {
        self.chan
            .exchange_declare(
                exchange,
                ExchangeKind::Headers,
                ExchangeDeclareOptions::default(),
                ft_default(),
            )
            .await
    }

    pub async fn declare_queue(&self, queue: &str) -> Result<(), lapin::Error> {
        self.chan
            .queue_declare(
                queue,
                QueueDeclareOptions::default(),
                ft_default(),
            )
            .await
            .map(|_| ())
    }

    pub async fn bind_queue(&self, queue: &str, exchange: &str) -> Result<(), lapin::Error> {
        self.chan
            .queue_bind(
                queue,
                exchange,
                ROUTING,
                QueueBindOptions::default(),
                ft_default(),
            )
            .await
    }

    pub async fn setup(&self) -> lapin::Result<()> {
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
            run_consumer(rabbit_delegator, consumer, self.chan.clone(), kill_signal)
                .instrument(Span::current()),
        ))
    }
}

#[derive(Error, Debug)]
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
    let (sender, receiver): (Sender<Delivery>, Receiver<_>) = async_channel::unbounded();

    let delegator = Arc::new(delegator);

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

        let Some(delivery) = delivery else {
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

    sender.close();

    for handle in handles {
        if let Err(err) = handle.await {
            error!("worker handle failure: {err}")
        }
    }
}

async fn worker<D: RabbitDelegator>(
    channel: Channel,
    mut receiver: Receiver<Delivery>,
    delegator: Arc<D>
) {
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

        async {
            let delegate_result = delegator.delegate(&header, contents).await;

            match delegate_result {
                Ok(_) => {
                    if let Err(err) = channel
                        .basic_ack(delivery_tag, BasicAckOptions::default())
                        .await
                    {
                        error!("failed to ack msg: {}", err);
                    }
                }
                Err(requeue) => {
                    let requeue = requeue.into();
                    error!("failed to delegate message - requeue = {requeue}");
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

// A trait for rabbit consumer/delegator errors
// that decides if a message should be requeued or not
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
// defaults to json deserialization, so it's required that the error type
// supports json errors
#[async_trait]
pub trait RabbitConsumer: Sync + Send + 'static {
    const MESSAGE_TYPE_HEADER: &'static str;

    type Message<'a>: Deserialize<'a> + Send;
    type ConsumerError: std::error::Error + From<serde_json::Error> + ShouldRequeue;

    fn parse_msg<'a>(&self, contents: &'a [u8]) -> Result<Self::Message<'a>, Self::ConsumerError> {
        serde_json::from_slice(contents).map_err(Into::into)
    }

    async fn process(&self, msg: Self::Message<'_>) -> Result<(), Self::ConsumerError>;

    async fn try_process(&self, contents: Vec<u8>) -> Result<(), Requeue> {
        self.try_process_inner(contents).await.map_err(|err| {
            error!("failed to process message: {}", err);
            ShouldRequeue::should_requeue(&err)
        })
    }

    async fn try_process_inner(&self, contents: Vec<u8>) -> Result<(), Self::ConsumerError> {
        let message = self.parse_msg(&contents)?;
        self.process(message).await
    }
}

// RabbitDelegator represents a thing which 'delegates' an incoming message
// to one of multiple consumers. the macro impls below create delegators of
// tuples of rabbit consumers & simply checks their headers match
// before passing it to the appropriate consumer
pub trait RabbitDelegator: Send + Sync + 'static {
    fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut;
}

#[pin_project(project=DelegateFutProj)]
pub enum DelegateFut<'a> {
    // we are using `async_trait` on consumers so this is unavoidable
    Fut(#[pin] BoxFuture<'a, Result<(), Requeue>>),
    Immediate(Requeue)
}

impl<'a> Future for DelegateFut<'a> {
    type Output = Result<(), Requeue>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            DelegateFutProj::Fut(f) => f.poll(cx),
            DelegateFutProj::Immediate(requeue) => Poll::Ready(Err(*requeue))
        }
    }
}

macro_rules! delegator_tuple {
    ( $ty:tt ) => {
        #[allow(unused_parens)]
        impl< $ty > RabbitDelegator for $ty
        where
            $ty: RabbitConsumer
        {
            fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut {
                if $ty::MESSAGE_TYPE_HEADER == header {
                    return DelegateFut::Fut(self.try_process(contents));
                }
                DelegateFut::Immediate(Requeue::No)
            }
        }

        #[allow(unused_parens)]
        #[async_trait]
        impl< $ty > RabbitDelegator for ($ty,)
        where
            $ty: RabbitConsumer
        {
            fn delegate(&self, header: &str, contents: Vec<u8>) -> DelegateFut {
                let (casey::lower!($ty),) = self;
                if $ty::MESSAGE_TYPE_HEADER == header {
                    return DelegateFut::Fut(casey::lower!($ty).try_process(contents));
                }
                DelegateFut::Immediate(Requeue::No)
            }
        }
    };
    ( $($ty:tt),* ) => {
        #[allow(unused_parens)]
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
                    return DelegateFut::Fut(casey::lower!($ty).try_process(contents));
                }
                )*
                DelegateFut::Immediate(Requeue::No)
            }
        }
    }
}

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
delegator_tuple!(A, B, C, D, E, F, G, H, I, J, K);
