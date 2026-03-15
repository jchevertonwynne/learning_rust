use std::net::SocketAddr;

use axum::{routing::get, Router};
use futures::FutureExt;
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{info, Level};

use tracing_showcase::{
    endpoints,
    fake_deck_of_cards_api_state::FakeDeckOfCardsAPIState,
    layers::{
        otlp_context_propagation::OtlpPropagatedTracingContextConsumerLayer,
        request_counter::RequestCounterLayer,
    },
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("fake deck of cards api")?;

    info!("hello!");

    let mongo_uri = std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let mongo_client = mongodb::Client::with_uri_str(mongo_uri).await?;
    let app_state = FakeDeckOfCardsAPIState::new(&mongo_client);

    info!("connected to mongo...");

    let router = Router::new()
        .route("/api/deck/new/shuffle/", get(endpoints::new_decks))
        .route("/api/deck/:deck_id/draw/", get(endpoints::draw_cards))
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                )
                .layer(OtlpPropagatedTracingContextConsumerLayer::new())
                .layer(RequestCounterLayer::new_for_http()),
        )
        .with_state(app_state);

    let shutdown = tokio::signal::ctrl_c().map(|_| ());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25566);
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    info!("serving on {addr}");

    let server = axum::Server::from_tcp(std::net::TcpListener::bind(addr)?)?
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown);

    server.await?;

    info!("goodbye!");

    Ok(())
}
