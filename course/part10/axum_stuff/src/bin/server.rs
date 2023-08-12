use std::net::{SocketAddr, TcpListener};

use axum::Server;
use futures::FutureExt;
use tracing::info;

use axum_stuff::routers::service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    info!("hello!");

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 25565)))?;
    let server = Server::from_tcp(listener)?
        .serve(
            service(),
            // tower::ServiceBuilder::new()
            // .load_shed()
            // .rate_limit(1, std::time::Duration::from_secs(1))
            // .layer(axum_stuff::tower_stuff::ConnectionLimitLayer::new(100))
            // .layer(axum_stuff::tower_stuff::NewConnSpanMakeServiceLayer)
            // .service(service()),
        )
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()));

    server.await?;

    info!("goodbye!");

    Ok(())
}
