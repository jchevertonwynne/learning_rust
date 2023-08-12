use std::net::SocketAddr;

use axum::{routing::get, Router};
use futures::FutureExt;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

use tracing_showcase::{
    endpoints,
    fake_deck_of_cards_api_state::FakeDeckOfCardsAPIState,
    layers::{
        jaeger_context_propagation::JaegerPropagatedTracingContextConsumerLayer,
        request_counter::{HttpChecker, RequestCounterLayer},
    },
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("fake deck of cards api")?;

    info!("hello!");

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let app_state = FakeDeckOfCardsAPIState::new(&mongo_client);

    info!("connected to mongo...");

    let router = Router::new()
        .route("/api/deck/new/shuffle/", get(endpoints::new_decks))
        .route("/api/deck/:deck_id/draw/", get(endpoints::draw_cards))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(JaegerPropagatedTracingContextConsumerLayer::new())
                .layer(RequestCounterLayer::new_for_http()),
        )
        .with_state(app_state);

    let shutdown = tokio::signal::ctrl_c().map(|_| ());

    let addr: SocketAddr = ([127, 0, 0, 1], 25566).into();
    info!("serving on {addr}");

    let server = axum::Server::from_tcp(std::net::TcpListener::bind(addr)?)?
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown);

    server.await?;

    info!("goodbye!");

    Ok(())
}
