use std::{
    net::{SocketAddr, TcpListener},
    time::Duration,
};

use axum::Server;
use axum_stuff::{routers::main_router, tower_stuff::NewConnTraceLayer};
use futures::FutureExt;
use tower::ServiceBuilder;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

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

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 25565)))?;
    let server = Server::from_tcp(listener)?
        .serve(
            ServiceBuilder::new()
                .layer(NewConnTraceLayer::default())
                // .concurrency_limit(5)
                .rate_limit(1, Duration::from_secs(5))
                .service(main_router().into_make_service()),
        )
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()));

    server.await?;

    info!("goodbye!");

    Ok(())
}
