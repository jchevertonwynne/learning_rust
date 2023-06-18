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

    let span2 = info_span!("connecting to server");
    let entered2 = span2.entered();

    let channel = tonic::transport::Endpoint::new("http://127.0.0.1:25565")?
        .connect()
        .await?;
    entered2.exit();
    let mut client = CardsServiceClient::with_interceptor(channel, intercept);

    let decks = client
        .new_decks(NewDecksRequest { decks: 5 })
        .instrument(info_span!("new decks request"))
        .await?
        .into_inner();

    let _drawn_hands = client
        .draw_cards(DrawCardsRequest {
            deck_id: decks.deck_id.clone(),
            count: 4,
            hands: 20,
        })
        .instrument(info_span!("draw hands request"))
        .await?
        .into_inner();

    // for hand in drawn_hands.hands {
    //     println!("{hand:#?}");
    // }

    entered.exit();

    info!("goodbye from the client!");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn intercept(mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
    let cx = tracing::Span::current().context();

    let inj = opentelemetry::global::get_text_map_propagator(|propagator| {
        let mut propagation_ctx = HashMap::<String, String>::default();
        propagator.inject_context(&cx, &mut propagation_ctx);
        propagation_ctx
    });

    let cx_string = match serde_json::to_string(&inj) {
        Ok(cx_string) => cx_string,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };

    let cx_string: MetadataValue<Ascii> = match cx_string.try_into() {
        Ok(cx_string) => cx_string,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };

    req.metadata_mut()
        .insert("tracing-parent-context", cx_string);

    Ok(req)
}
