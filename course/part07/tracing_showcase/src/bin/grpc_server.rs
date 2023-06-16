// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use futures::{FutureExt, StreamExt};
use grpc::cards_service_server::CardsServiceServer;
use grpc::{DrawCardsRequest, DrawCardsResponse, NewDecksRequest, NewDecksResponse};
use mongodb::bson::doc;
use mongodb::options::UpdateModifications;
use mongodb::Collection;
use serde::{Deserialize, Serialize};
use tracing::{info, trace};
use tracing_showcase::deck_of_cards::DeckID;
use tracing_showcase::deck_of_cards::{self, DrawnCardsInfo};
use tracing_showcase::grpc;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Registry, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name("tracing_showcase")
        .with_max_packet_size(9216)
        .with_auto_split_batch(true)
        .install_batch(opentelemetry::runtime::Tokio)?;

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry)
        .init();

    info!("starting grpc server...");

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let collection = mongo_client.database("joseph").collection("testing");
    let record_controller = MongoRecordController::new(collection);

    info!("connected to mongo...");

    let client = reqwest::ClientBuilder::default().build()?;

    let service = CardsServiceState::new(client, record_controller);

    let addr = ([127, 0, 0, 1], 25565).into();

    info!("serving on {addr}");

    let shutdown = tokio::signal::ctrl_c().map(|_| ());
    tonic::transport::Server::builder()
        .add_service(CardsServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    info!("this is another log!");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

#[tracing::instrument(name = "my instrumented layer")]
fn task(a: usize, b: usize) -> usize {
    trace!("logging from the task");
    a + b
}

struct CardsServiceState {
    requests: AtomicUsize,
    client: reqwest::Client,
    record_controller: MongoRecordController,
}

impl CardsServiceState {
    fn new(client: reqwest::Client, record_controller: MongoRecordController) -> Self {
        let requests = AtomicUsize::default();
        Self {
            requests,
            client,
            record_controller,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    deck_id: String,
    count: usize,
}

#[async_trait]
impl grpc::cards_service_server::CardsService for CardsServiceState {
    #[tracing::instrument(skip(self, request))]
    async fn new_decks(
        &self,
        request: tonic::Request<NewDecksRequest>,
    ) -> Result<tonic::Response<NewDecksResponse>, tonic::Status> {
        let requests = 1 + self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        info!("there have been {requests} requests");

        let deck_info =
            match deck_of_cards::new_deck(self.client.clone(), request.into_inner().decks as usize)
                .await
            {
                Ok(deck_info) => deck_info,
                Err(err) => return Err(tonic::Status::internal(err.to_string())),
            };

        info!("created a new deck");

        if let Err(err) = self.record_controller.create(deck_info.deck_id).await {
            return Err(tonic::Status::internal(err.to_string()));
        }

        info!("stored deck in mongo");

        let deck_id = deck_info.deck_id.to_string();

        Ok(tonic::Response::new(NewDecksResponse { deck_id }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn draw_cards(
        &self,
        request: tonic::Request<DrawCardsRequest>,
    ) -> Result<tonic::Response<DrawCardsResponse>, tonic::Status> {
        let requests = 1 + self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        info!("there have been {requests} requests");

        let DrawCardsRequest {
            deck_id,
            count,
            hands,
        } = request.into_inner();
        let deck_id = match DeckID::try_from(deck_id.as_str()) {
            Ok(deck_id) => deck_id,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        if count <= 0 {
            return Err(tonic::Status::invalid_argument(
                "count must be greater than or equal to zero",
            ));
        }

        let hands = match draw_all_cards(self.client.clone(), deck_id, hands, count).await {
            Ok(hands) => hands,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        info!("drawn all cards");

        if let Err(err) = self.record_controller.increment_count(deck_id).await {
            return Err(tonic::Status::internal(err.to_string()));
        }

        info!("incremented count in mongo");

        let hands = hands
            .into_iter()
            .map(|hand| {
                let cards = hand.cards.iter().map(grpc::Card::from).collect();
                grpc::Hand { cards }
            })
            .collect();

        Ok(tonic::Response::new(DrawCardsResponse { hands }))
    }
}

#[tracing::instrument(skip(client))]
async fn draw_all_cards(
    client: reqwest::Client,
    deck_id: DeckID,
    hands: i32,
    count: i32,
) -> Result<Vec<DrawnCardsInfo>, reqwest::Error> {
    let mut stream = futures::stream::iter((0..hands).map(|_| {
        deck_of_cards::draw_cards(client.clone(), deck_id, count as u8)
            .expect("we checked the count is >0")
    }))
    .buffer_unordered(3);

    let mut hands = vec![];
    while let Some(hand) = stream.next().await {
        match hand {
            Ok(hand) => hands.push(hand),
            Err(err) => return Err(err),
        }
    }

    Ok(hands)
}

struct MongoRecordController {
    collection: Collection<Record>,
}

impl MongoRecordController {
    fn new(collection: Collection<Record>) -> Self {
        Self { collection }
    }

    #[tracing::instrument(skip(self))]
    async fn create(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("creating a new record");
        self.collection
            .insert_one(
                Record {
                    deck_id: deck_id.to_string(),
                    count: 0,
                },
                None,
            )
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn increment_count(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("incrementing count");
        self.collection
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                UpdateModifications::Document(doc! { "$inc": { "count": 1 } }),
                None,
            )
            .await?;
        Ok(())
    }
}
