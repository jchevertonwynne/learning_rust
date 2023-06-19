use std::net::ToSocketAddrs;

use anyhow::Context;
use axum::routing::get;
use axum::Router;
use futures::FutureExt;
use tracing::info;
use tracing_showcase::endpoints;

use tracing_showcase::deck_of_cards_api_state::DeckOfCardsAPIState;
use tracing_showcase::tracing_setup::init_tracing;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("fake deck of cards api")?;

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let app_state = DeckOfCardsAPIState::new(&mongo_client);

    info!("connected to mongo...");

    let router = Router::new()
        .route("/api/deck/new/shuffle/", get(endpoints::new_decks))
        .route("/api/deck/:deck_id/draw/", get(endpoints::draw_cards))
        .with_state(app_state);

    let shutdown = tokio::signal::ctrl_c().map(|_| ());

    let addr = "127.0.0.1:25566"
        .to_socket_addrs()?
        .next()
        .context("expected an address")?;
    info!("serving on {addr}");

    let server = axum::Server::from_tcp(std::net::TcpListener::bind(addr)?)?
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown);

    server.await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}
