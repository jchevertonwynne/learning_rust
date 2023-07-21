use std::sync::{atomic::AtomicUsize, Arc};

use futures::FutureExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

use rabbit_stuff::{
    impls::{MyMessageConsumer, OtherMessageConsumer},
    rabbit::{Rabbit, QUEUE},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    info!("hello!");

    let rabbit = Rabbit::new("amqp://localhost:5672").await?;
    rabbit.setup().await?;

    let cancel = CancellationToken::new();

    let global_counter = Arc::new(AtomicUsize::new(0));

    let rabbit_consumer_handle = rabbit
        .consume(
            QUEUE,
            (
                MyMessageConsumer::new(global_counter.clone()),
                // OtherMessageConsumer::new(global_counter),
            ),
            cancel.clone(),
        )
        .await?;

    tokio::signal::ctrl_c().map(|_| ()).await;

    info!("shutting down!");

    cancel.cancel();

    rabbit_consumer_handle.await?;

    info!("shut down rabbit consumer");

    rabbit.close().await?;

    info!("goodbye!");

    Ok(())
}
