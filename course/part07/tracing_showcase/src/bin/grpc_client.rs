use grpc::cards_service_client::CardsServiceClient;
use opentelemetry::global;
use opentelemetry::propagation::{Injector, TextMapPropagator};
use opentelemetry::sdk::propagation::TraceContextPropagator;
use serde::Serialize;
use std::collections::HashMap;
use tracing::{info, info_span, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_showcase::grpc::{DrawCardsRequest, NewDecksRequest};
use tracing_showcase::{grpc, init_tracing};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cleanup = init_tracing("grpc caller")?;

    let span = info_span!("being a client");
    let _enter = span.enter();

    let mut client = CardsServiceClient::connect("http://127.0.0.1:25565").await?;

    let cx = span.context();
    let mut inj = MyContextInjector::inject(&cx);
    info!("injector = {inj:?}");

    let t = TraceContextPropagator::new();
    t.inject(&mut inj);
    let cx_string = serde_json::to_string(&inj)?;
    info!("created ct string: {cx_string}");

    let decks = client
        .new_decks(NewDecksRequest {
            decks: 5,
            ctx: cx_string.clone(),
        })
        .instrument(info_span!("new decks request"))
        .await?
        .into_inner();

    let drawn_hands = client
        .draw_cards(DrawCardsRequest {
            deck_id: decks.deck_id.clone(),
            count: 4,
            hands: 20,
            ctx: cx_string.clone(),
        })
        .instrument(info_span!("draw hands request"))
        .await?
        .into_inner();

    for hand in drawn_hands.hands {
        println!("{hand:#?}");
    }

    Ok(())
}

#[derive(Debug, Default, Serialize)]
struct MyContextInjector(HashMap<String, String>);

impl MyContextInjector {
    fn inject(context: &opentelemetry::Context) -> Self {
        global::get_text_map_propagator(|propagator| {
            let mut propagation_ctx = MyContextInjector::default();
            propagator.inject_context(context, &mut propagation_ctx);
            propagation_ctx
        })
    }
}

impl Injector for MyContextInjector {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}
