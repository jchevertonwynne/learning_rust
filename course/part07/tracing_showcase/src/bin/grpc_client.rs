use tower::ServiceBuilder;
use tracing::{info, info_span, instrument, Instrument};

use tracing_showcase::{
    grpc::proto::{cards_service_client::CardsServiceClient, DrawCardsRequest, NewDecksRequest},
    layers::{
        jaeger_context_propagation::JaegerPropagatedTracingContextProducerLayer,
        request_counter::{GrpcCheckRequest, RequestCounterLayer},
    },
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("grpc caller")?;

    info!("hello from the client!");

    let res = run_client().await;

    info!("goodbye from the client!");

    res
}

#[instrument]
async fn run_client() -> anyhow::Result<()> {
    let channel = tonic::transport::Endpoint::new("http://127.0.0.1:25565")?
        .connect()
        .instrument(info_span!("connecting to server"))
        .await?;

    let client = tower::ServiceBuilder::new()
        .layer(
            ServiceBuilder::new()
                .layer(JaegerPropagatedTracingContextProducerLayer)
                .layer(RequestCounterLayer::new(GrpcCheckRequest::new())),
        )
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
            count: 5,
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
