use std::{
    net::{SocketAddr, TcpListener},
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use axum::Server;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tracing::info;

use axum_stuff::{
    routers::service,
    tower_stuff::{ConnectionLimitLayer, NewConnSpanMakeServiceLayer},
};
use rabbit_stuff::{
    impls::{MyMessageConsumer, OtherMessageConsumer},
    rabbit::{Rabbit, QUEUE},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

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
                OtherMessageConsumer::new(global_counter),
            ),
            cancel.clone(),
        )
        .await?;

    info!("set up rabbit connection!");

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 25565)))?;

    let addr = listener.local_addr()?;
    info!("accepting requests on  {addr:?}");
    let server = Server::from_tcp(listener)?
        .serve(
            // service(),
            ServiceBuilder::new()
                .load_shed()
                .rate_limit(87654321, Duration::from_secs(1))
                .layer(ConnectionLimitLayer::new(12345678))
                .layer(NewConnSpanMakeServiceLayer)
                .service(service(rabbit)),
        )
        .with_graceful_shutdown({
            let shutdown = cancel.clone();
            async move { shutdown.cancelled().await }
        });

    let server_handle = tokio::spawn(server);

    let _ = tokio::signal::ctrl_c().await;

    cancel.cancel();

    server_handle.await??;

    rabbit_consumer_handle.await?;

    info!("goodbye!");

    Ok(())
}
