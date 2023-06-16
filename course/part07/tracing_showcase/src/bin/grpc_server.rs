// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use futures::{FutureExt, StreamExt};
use grpc::cards_service_server::CardsServiceServer;
use mongodb::bson::doc;
use mongodb::options::UpdateModifications;
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_showcase::deck_of_cards::{self, DrawnCardsInfo};
use tracing_showcase::deck_of_cards::{DeckID, DeckInfo};
use tracing_showcase::grpc;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_opentelemetry::layer().with_tracer(
                opentelemetry_jaeger::new_agent_pipeline()
                    .with_service_name("tracing_showcase")
                    .with_max_packet_size(8192)
                    .with_auto_split_batch(true)
                    .install_batch(opentelemetry::runtime::Tokio)?,
            ),
        )
        .init();

    info!("starting grpc server...");

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let record_controller = MongoRecordController::new(&mongo_client);

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

#[async_trait]
impl grpc::cards_service_server::CardsService for CardsServiceState {
    #[tracing::instrument(skip(self, request))]
    async fn new_decks(
        &self,
        request: tonic::Request<grpc::NewDecksRequest>,
    ) -> Result<tonic::Response<grpc::NewDecksResponse>, tonic::Status> {
        let requests = 1 + self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        info!("there have been {requests} requests");

        let NewDecksRequest { decks } = match NewDecksRequest::try_from(request.into_inner()) {
            Ok(deck_request) => deck_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let deck_id = match self._new_deck(decks).await {
            Ok(deck_id) => deck_id,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        Ok(tonic::Response::new(grpc::NewDecksResponse {
            deck_id: deck_id.to_string(),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn draw_cards(
        &self,
        request: tonic::Request<grpc::DrawCardsRequest>,
    ) -> Result<tonic::Response<grpc::DrawCardsResponse>, tonic::Status> {
        let requests = 1 + self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        info!("there have been {requests} requests");

        let DrawCardsRequest {
            deck_id,
            count,
            hands,
        } = match DrawCardsRequest::try_from(request.into_inner()) {
            Ok(cards_request) => cards_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let hands = match self._draw_cards(deck_id, hands, count).await {
            Ok(hands) => hands,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        let hands = hands
            .into_iter()
            .map(|hand| {
                let cards = hand.cards.iter().map(grpc::Card::from).collect();
                grpc::Hand { cards }
            })
            .collect();

        Ok(tonic::Response::new(grpc::DrawCardsResponse { hands }))
    }
}

#[derive(Debug, thiserror::Error)]
enum NewDeckError {
    #[error("failed to draw cards: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug, thiserror::Error)]
enum DrawCardsError {
    #[error("failed to draw cards: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

impl CardsServiceState {
    #[tracing::instrument(skip(self))]
    async fn _new_deck(&self, decks: usize) -> Result<DeckID, NewDeckError> {
        let DeckInfo { deck_id, .. } = deck_of_cards::new_deck(self.client.clone(), decks).await?;

        info!("created a new deck");

        self.record_controller.create(deck_id).await?;

        info!("stored deck in mongo");

        Ok(deck_id)
    }

    #[tracing::instrument(skip(self))]
    async fn _draw_cards(
        &self,
        deck_id: DeckID,
        hands: usize,
        count: u8,
    ) -> Result<Vec<DrawnCardsInfo>, DrawCardsError> {
        let hands = draw_all_cards(self.client.clone(), deck_id, hands, count).await?;

        info!("drawn all cards");

        self.record_controller.increment_count(deck_id).await?;

        info!("incremented count in mongo");

        Ok(hands)
    }
}

struct NewDecksRequest {
    decks: usize,
}

#[derive(Debug, thiserror::Error)]
enum NewDecksRequestValidationError {
    #[error("count must be a positive integer")]
    InvalidDeckCount,
}

impl TryFrom<grpc::NewDecksRequest> for NewDecksRequest {
    type Error = NewDecksRequestValidationError;

    fn try_from(value: grpc::NewDecksRequest) -> Result<Self, Self::Error> {
        let grpc::NewDecksRequest { decks } = value;

        let Ok(decks) = usize::try_from(decks) else {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        };

        if decks == 0 {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        }

        Ok(NewDecksRequest { decks })
    }
}

struct DrawCardsRequest {
    deck_id: DeckID,
    hands: usize,
    count: u8,
}

#[derive(Debug, thiserror::Error)]
enum DrawCardsRequestValidationError {
    #[error("a deck id must be 12 lowercase letters and numbers")]
    DeckID,
    #[error("hands must be a positive integer")]
    Hands,
    #[error("count must be a positive u8 value")]
    Count,
}

impl TryFrom<grpc::DrawCardsRequest> for DrawCardsRequest {
    type Error = DrawCardsRequestValidationError;

    fn try_from(value: grpc::DrawCardsRequest) -> Result<Self, Self::Error> {
        let grpc::DrawCardsRequest {
            deck_id,
            hands,
            count,
        } = value;

        let Ok(deck_id) = DeckID::try_from(deck_id.as_str()) else {
            return Err(DrawCardsRequestValidationError::DeckID);
        };

        let Ok(count) =  u8::try_from(count) else {
            return Err(DrawCardsRequestValidationError::Count);
        };

        let Ok(hands) = usize::try_from(hands) else {
            return Err(DrawCardsRequestValidationError::Hands);
        };

        Ok(DrawCardsRequest {
            deck_id,
            hands,
            count,
        })
    }
}

#[tracing::instrument(skip(client))]
async fn draw_all_cards(
    client: reqwest::Client,
    deck_id: DeckID,
    hands: usize,
    count: u8,
) -> Result<Vec<DrawnCardsInfo>, reqwest::Error> {
    let mut stream = futures::stream::iter(
        (0..hands).map(|_| deck_of_cards::draw_cards(client.clone(), deck_id, count)),
    )
    .buffer_unordered(3);

    let mut hands = vec![];
    while let Some(hand) = stream.next().await {
        hands.push(hand?);
    }

    Ok(hands)
}

#[derive(Serialize, Deserialize)]
struct Record {
    deck_id: String,
    count: usize,
}

struct MongoRecordController {
    collection: mongodb::Collection<Record>,
}

impl MongoRecordController {
    fn new(client: &mongodb::Client) -> Self {
        let collection = client.database("joseph").collection("testing");
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
