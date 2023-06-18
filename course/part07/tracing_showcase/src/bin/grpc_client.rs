use grpc::cards_service_client::CardsServiceClient;
use std::collections::HashMap;

use tonic::metadata::{Ascii, MetadataValue};

use tracing::{info, info_span, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_showcase::grpc::{DrawCardsRequest, NewDecksRequest};
use tracing_showcase::{grpc, init_tracing};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("grpc caller")?;

    info!("hello from the client!");

    let span = info_span!("being a client");
    let entered = span.entered();

    let channel = tonic::transport::Endpoint::new("http://127.0.0.1:25565")?
        .connect()
        .instrument(info_span!("connecting to server"))
        .await?;
    let mut client = CardsServiceClient::with_interceptor(channel, intercept);

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

    entered.exit();

    info!("goodbye from the client!");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn intercept(mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
    let ctx = tracing::Span::current().context();

    let ctx_map = opentelemetry::global::get_text_map_propagator(|propagator| {
        let mut propagation_ctx = HashMap::<String, String>::default();
        propagator.inject_context(&ctx, &mut propagation_ctx);
        propagation_ctx
    });
    
    let ctx_str = match serde_json::to_string(&ctx_map) {
        Ok(ctx_str) => ctx_str,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };
    
    let ctx_str: MetadataValue<Ascii> = match ctx_str.try_into() {
        Ok(ctx_str) => ctx_str,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };
    
    req.metadata_mut().insert("tracing-parent-context", ctx_str);

    Ok(req)
}
