use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::rabbit::{RabbitConsumer, Requeue, ShouldRequeue, MESSAGE_TYPE, MESSAGE_TYPE_2};

#[derive(Debug, Default)]
pub struct MyMessageConsumer {
    received: AtomicUsize,
    received_all: Arc<AtomicUsize>,
}

impl MyMessageConsumer {
    pub fn new(received_all: Arc<AtomicUsize>) -> MyMessageConsumer {
        MyMessageConsumer {
            received: Default::default(),
            received_all,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct MyMessage<'a> {
    pub age: usize,
    #[serde(borrow)]
    pub name: Cow<'a, str>,
}

#[derive(Debug, thiserror::Error)]
pub enum MyMessageConsumerError {
    #[error("failed to parse json: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("arbitrary error: total count {0} < 5")]
    ArbitraryError(usize),
}

impl ShouldRequeue for MyMessageConsumerError {
    fn should_requeue(&self) -> Requeue {
        match self {
            MyMessageConsumerError::JsonError(_) => Requeue::No,
            MyMessageConsumerError::ArbitraryError(_) => Requeue::Yes,
        }
    }
}

#[async_trait]
impl RabbitConsumer for MyMessageConsumer {
    const MESSAGE_TYPE_HEADER: &'static str = MESSAGE_TYPE;

    type Message<'a> = MyMessage<'a>;
    type ConsumerError = MyMessageConsumerError;

    async fn process(&self, msg: Self::Message<'_>) -> Result<(), Self::ConsumerError> {
        let msgs_received = self.received.fetch_add(1, SeqCst) + 1;
        let total_msgs_received = self.received_all.fetch_add(1, SeqCst) + 1;

        if msgs_received < 5 {
            return Err(MyMessageConsumerError::ArbitraryError(msgs_received));
        }

        let is_borrowed = matches!(msg.name, Cow::Borrowed(_));

        info!("got message #{msgs_received}: {msg:?} - name is borrowed = {is_borrowed} - total processed = {total_msgs_received}");

        Ok(())
    }
}

#[derive(Debug)]
pub struct OtherMessageConsumer {
    received: AtomicUsize,
    received_all: Arc<AtomicUsize>,
    pupils: Mutex<Vec<Pupil>>,
}

impl OtherMessageConsumer {
    pub fn new(received_all: Arc<AtomicUsize>) -> OtherMessageConsumer {
        OtherMessageConsumer {
            received: Default::default(),
            received_all,
            pupils: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OtherMessage {
    pub school_age: SchoolAge,
    pub pupils: Vec<Pupil>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum SchoolAge {
    Primary,
    Secondary,
    Other,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pupil {
    pub first_name: String,
    pub second_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OtherMessageError {
    #[error("failed to parse json: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl ShouldRequeue for OtherMessageError {}

#[async_trait]
impl RabbitConsumer for OtherMessageConsumer {
    const MESSAGE_TYPE_HEADER: &'static str = MESSAGE_TYPE_2;

    type Message<'a> = OtherMessage;
    type ConsumerError = OtherMessageError;

    async fn process(&self, msg: Self::Message<'_>) -> Result<(), Self::ConsumerError> {
        let msgs_received = self.received.fetch_add(1, SeqCst) + 1;
        let total_msgs_received = self.received_all.fetch_add(1, SeqCst) + 1;

        info!("got message #{msgs_received}: {msg:?} - total processed = {total_msgs_received}");

        let mut pupils = self.pupils.lock().await;
        pupils.extend(msg.pupils);

        info!("there are a total {} pupils", pupils.len());

        Ok(())
    }
}
