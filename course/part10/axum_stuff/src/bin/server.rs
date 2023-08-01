use std::{
    net::{SocketAddr, TcpListener},
    time::Duration,
};

use axum::Server;
use axum_stuff::{
    routers::main_router,
    tower_stuff::{ConnectionLimitLayer, NewConnSpanMakeServiceLayer},
};
use futures::FutureExt;
use tower::ServiceBuilder;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    info!("hello!");

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 25565)))?;
    let server = Server::from_tcp(listener)?
        .serve(
            // main_router().into_make_service(),
            ServiceBuilder::new()
                // .load_shed()
                .rate_limit(1, Duration::from_secs(1))
                .layer(ConnectionLimitLayer::new(10))
                .layer(NewConnSpanMakeServiceLayer)
                .service(main_router().into_make_service()),
        )
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()));

    server.await?;

    info!("goodbye!");

    Ok(())
}
