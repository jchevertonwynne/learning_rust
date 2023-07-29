use axum::handler::HandlerWithoutStateExt;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 25565));

    axum::Server::bind(&addr)
        .serve((|| async { "hello world" }).into_make_service())
        .await?;

    Ok(())
}
