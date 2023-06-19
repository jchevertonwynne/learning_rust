use tracing::{info, info_span, instrument, Instrument};

use tracing_showcase::grpc::proto::cards_service_client::CardsServiceClient;
use tracing_showcase::grpc::proto::{DrawCardsRequest, NewDecksRequest};
use tracing_showcase::layers::{inject_jaeger_context, GrpcCheckSuccess};
use tracing_showcase::{layers::RequestCounterLayer, tracing_setup::init_tracing};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("grpc caller")?;

    info!("hello from the client!");

    let res = run_client().await;

    info!("goodbye from the client!");

    opentelemetry::global::shutdown_tracer_provider();

    res
}

#[instrument]
async fn run_client() -> anyhow::Result<()> {
    let channel = tonic::transport::Endpoint::new("http://127.0.0.1:25565")?
        .connect()
        .instrument(info_span!("connecting to server"))
        .await?;

    let client = tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor(inject_jaeger_context))
        .layer(RequestCounterLayer::new(GrpcCheckSuccess::new()))
        .service(channel);
    let mut client = CardsServiceClient::new(client);

    let decks = client
        .new_decks(NewDecksRequest { decks: 5 })
        .instrument(info_span!("new decks request"))
        .await?
        .into_inner();

    let drawn_hands = client
        .draw_cards(DrawCardsRequest {
            deck_id: decks.deck_id.clone(),
            count: 4,
            hands: 20,
        })
        .instrument(info_span!("draw hands request"))
        .await?
        .into_inner();

    let cards = drawn_hands
        .hands
        .iter()
        .flat_map(|hand| hand.cards.iter())
        .count();

    info!("retrieved {cards} cards");

    Ok(())
}
