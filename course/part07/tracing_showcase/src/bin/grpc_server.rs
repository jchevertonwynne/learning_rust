// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

// DECK_OF_CARDS_URL=http://localhost:25566 to use fake deck of cards api

use futures::FutureExt;
use hyper::Client as HyperClient;
use mongodb::Client as MongoClient;
use tonic::transport::Server;
use tower::{limit::ConcurrencyLimitLayer, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing::info;
use url::Url;

use tracing_showcase::{
    deck_of_cards::DeckOfCardsClient,
    grpc::{proto::cards_service_server::CardsServiceServer, CardsService},
    layers::{
        jaeger_context_propagation::{
            JaegerPropagatedTracingContextConsumerLayer,
            JaegerPropagatedTracingContextProducerLayer,
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

    let mongo_client = MongoClient::with_uri_str("mongodb://localhost:27017").await?;
    let record_controller = MongoRecordController::new(&mongo_client);

    info!("connected to mongo...");

    let client = ServiceBuilder::new()
        .layer(ConcurrencyLimitLayer::new(10))
        .layer(JaegerPropagatedTracingContextProducerLayer)
        .service(HyperClient::builder().http2_only(true).build_http());
    let url = Url::try_from(
        std::env::var("DECK_OF_CARDS_URL")
            .unwrap_or("https://deckofcardsapi.com".to_string())
            .as_str(),
    )?;
    info!("deck of cards url = {url:?}");
    let cards_client = DeckOfCardsClient::new(url, client);

    let service = CardsService::new(cards_client, record_controller);

    let addr = ([127, 0, 0, 1], 25565).into();

    info!("serving on {addr}");

    let shutdown = tokio::signal::ctrl_c().map(|_| ());
    Server::builder()
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_grpc())
                .layer(JaegerPropagatedTracingContextConsumerLayer::new())
                .layer(RequestCounterLayer::new_for_http()),
        )
        .add_service(CardsServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    info!("goodbye!");

    Ok(())
}
