// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

// DECK_OF_CARDS_URL=http://localhost:25566 to use fake deck of cards api

use futures::FutureExt;
use hyper::Client as HyperClient;
use mongodb::Client as MongoClient;
use std::time::Duration;
use tonic::transport::Server;
use tower::ServiceBuilder;
use tower_http::{
    decompression::DecompressionLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{info, Level};
use url::Url;

use tracing_showcase::{
    deck_of_cards::DeckOfCardsClient,
    grpc::{proto::cards_service_server::CardsServiceServer, CardsService},
    layers::{
        otlp_context_propagation::{
            OtlpPropagatedTracingContextConsumerLayer,
            OtlpPropagatedTracingContextProducerLayer,
        },
        request_counter::RequestCounterLayer,
    },
    mongo::MongoRecordController,
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("grpc server")?;

    info!("starting grpc server...");

    let mongo_uri = std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let mongo_client = MongoClient::with_uri_str(mongo_uri).await?;
    let record_controller = MongoRecordController::new(&mongo_client);

    info!("connected to mongo...");

    let client = ServiceBuilder::new()
        .rate_limit(100, Duration::from_secs(1))
    
        .layer(DecompressionLayer::new())
        .layer(OtlpPropagatedTracingContextProducerLayer)
        .service(HyperClient::builder().build_http());
    let url = Url::try_from(
        std::env::var("DECK_OF_CARDS_URL")
            .unwrap_or("https://deckofcardsapi.com".to_string())
            .as_str(),
    )?;
    info!("deck of cards url = {url:?}");
    let cards_client = DeckOfCardsClient::new(url, client);

    let service = CardsService::new(cards_client, record_controller);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25565);
    let addr = ([0, 0, 0, 0], port).into();

    info!("serving on {addr}");

    let shutdown = tokio::signal::ctrl_c().map(|_| ());
    Server::builder()
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_grpc()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                )
                .layer(OtlpPropagatedTracingContextConsumerLayer::new())
                .layer(RequestCounterLayer::new_for_grpc()),
        )
        .add_service(CardsServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    info!("goodbye!");

    Ok(())
}
