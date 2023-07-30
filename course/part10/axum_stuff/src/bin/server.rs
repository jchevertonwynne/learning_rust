use std::{
    net::{SocketAddr, TcpListener},
    time::Duration,
};

use axum::Server;
use axum_stuff::{routers::main_router, tower_stuff::NewConnSpanMakeServiceLayer};
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
                .layer(NewConnSpanMakeServiceLayer)
                // .load_shed()
                // .concurrency_limit(5)
                // .rate_limit(50, Duration::from_millis(100))
                .service(main_router().into_make_service()),
        )
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()));

    server.await?;

    info!("goodbye!");

    Ok(())
}
