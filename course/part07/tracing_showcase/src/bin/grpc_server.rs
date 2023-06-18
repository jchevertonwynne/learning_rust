// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

// DECK_OF_CARDS_URL=http://localhost:25566 to use fake deck of cards api

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use futures::{FutureExt, StreamExt, TryStreamExt};
use grpc::cards_service_server::CardsServiceServer;
use mongodb::bson::doc;
use mongodb::options::UpdateModifications;
use opentelemetry::propagation::Extractor;
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_showcase::deck_of_cards::{DeckID, DeckInfo};
use tracing_showcase::deck_of_cards::{DeckOfCardsClient, DrawnCardsInfo};
use tracing_showcase::{grpc, init_tracing};
use url::Url;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    init_tracing("grpc server")?;

    info!("starting grpc server...");

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let record_controller = MongoRecordController::new(&mongo_client);

    info!("connected to mongo...");

    let client = reqwest::ClientBuilder::default().build()?;
    let url = Url::try_from(
        std::env::var("DECK_OF_CARDS_URL")
            .unwrap_or("https://deckofcardsapi.com".to_string())
            .as_str(),
    )?;
    let cards_client = DeckOfCardsClient::new(url, client);

    let service = CardsServiceState::new(cards_client, record_controller);

    let addr = ([127, 0, 0, 1], 25565).into();

    info!("serving on {addr}");

    let shutdown = tokio::signal::ctrl_c().map(|_| ());
    tonic::transport::Server::builder()
        .add_service(CardsServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    info!("goodbye!");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

struct CardsServiceState {
    cards_client: DeckOfCardsClient,
    requests: AtomicUsize,
    record_controller: MongoRecordController,
}

impl CardsServiceState {
    fn new(cards_client: DeckOfCardsClient, record_controller: MongoRecordController) -> Self {
        let requests = AtomicUsize::default();
        Self {
            cards_client,
            requests,
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

        let WithContext::<NewDecksRequest> {
            thing: new_decks_request,
            ctx,
        } = match WithContext::<NewDecksRequest>::try_from(request.into_inner()) {
            Ok(deck_request) => deck_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };
        tracing::Span::current().set_parent(ctx);

        let deck_id = match self._new_deck(new_decks_request).await {
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

        let WithContext::<DrawCardsRequest> {
            thing: draw_cards_request,
            ctx,
        } = match WithContext::<DrawCardsRequest>::try_from(request.into_inner()) {
            Ok(cards_request) => cards_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };
        tracing::Span::current().set_parent(ctx);

        let hands = match self._draw_cards(draw_cards_request).await {
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
    #[error("failed to draw deck: {0}")]
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
    async fn _new_deck(&self, new_decks_request: NewDecksRequest) -> Result<DeckID, NewDeckError> {
        let NewDecksRequest { decks } = new_decks_request;

        let DeckInfo { deck_id, .. } = self.cards_client.new_deck(decks).await?;

        info!("created a new deck");

        self.record_controller.create(deck_id).await?;

        info!("stored deck in mongo");

        Ok(deck_id)
    }

    #[tracing::instrument(skip(self))]
    async fn _draw_cards(
        &self,
        draw_cards_request: DrawCardsRequest,
    ) -> Result<Vec<DrawnCardsInfo>, DrawCardsError> {
        let DrawCardsRequest {
            deck_id,
            hands,
            count,
        } = draw_cards_request;

        let hands = self.draw_all_cards(deck_id, hands, count).await?;

        info!("drawn all cards");

        self.record_controller.increment_count(deck_id).await?;

        info!("incremented count in mongo");

        Ok(hands)
    }

    #[tracing::instrument(skip(self))]
    async fn draw_all_cards(
        &self,
        deck_id: DeckID,
        hands: usize,
        count: u8,
    ) -> Result<Vec<DrawnCardsInfo>, reqwest::Error> {
        // (0..hands)
        //     .map(|_| self.cards_client.draw_cards(deck_id, count))
        //     .collect::<FuturesUnordered<_>>()
        futures::stream::iter((0..hands).map(|_| self.cards_client.draw_cards(deck_id, count)))
            .buffer_unordered(5)
            .try_collect()
            .await
    }
}

struct WithContext<T> {
    thing: T,
    ctx: opentelemetry::Context,
}

#[derive(Debug)]
struct NewDecksRequest {
    decks: usize,
}

#[derive(Debug, thiserror::Error)]
enum NewDecksRequestValidationError {
    #[error("count must be a positive integer")]
    InvalidDeckCount,
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<grpc::NewDecksRequest> for WithContext<NewDecksRequest> {
    type Error = NewDecksRequestValidationError;

    fn try_from(value: grpc::NewDecksRequest) -> Result<Self, Self::Error> {
        let grpc::NewDecksRequest { decks, ctx } = value;

        info!("received context string: {ctx}");

        let ext: MyContextExtractor = serde_json::from_str(&ctx)?;
        info!("extracted to: {ext:?}");
        let ctx = ext.extract();

        let Ok(decks) = usize::try_from(decks) else {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        };

        if decks == 0 {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        }

        Ok(WithContext {
            thing: NewDecksRequest { decks },
            ctx,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct MyContextExtractor(HashMap<String, String>);

impl MyContextExtractor {
    fn extract(&self) -> opentelemetry::Context {
        opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}

impl Extractor for MyContextExtractor {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|v| v.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_ref()).collect()
    }
}

#[derive(Debug)]
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
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<grpc::DrawCardsRequest> for WithContext<DrawCardsRequest> {
    type Error = DrawCardsRequestValidationError;

    fn try_from(value: grpc::DrawCardsRequest) -> Result<Self, Self::Error> {
        let grpc::DrawCardsRequest {
            deck_id,
            hands,
            count,
            ctx,
        } = value;

        info!("received context string: {ctx}");

        let ext: MyContextExtractor = serde_json::from_str(&ctx)?;
        let ctx = ext.extract();

        let Ok(deck_id) = DeckID::try_from(deck_id.as_str()) else {
            return Err(DrawCardsRequestValidationError::DeckID);
        };

        let Ok(count) =  u8::try_from(count) else {
            return Err(DrawCardsRequestValidationError::Count);
        };

        let Ok(hands) = usize::try_from(hands) else {
            return Err(DrawCardsRequestValidationError::Hands);
        };

        Ok(WithContext {
            thing: DrawCardsRequest {
                deck_id,
                hands,
                count,
            },
            ctx,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct InteractionRecord {
    deck_id: String,
    count: usize,
}

struct MongoRecordController {
    interactions: mongodb::Collection<InteractionRecord>,
}

impl MongoRecordController {
    fn new(client: &mongodb::Client) -> Self {
        let collection = client
            .database("tracing_showcase")
            .collection("interactions");
        Self {
            interactions: collection,
        }
    }

    #[tracing::instrument(skip(self))]
    async fn create(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("creating a new record");
        self.interactions
            .insert_one(
                InteractionRecord {
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
        self.interactions
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                UpdateModifications::Document(doc! { "$inc": { "count": 1 } }),
                None,
            )
            .await?;
        Ok(())
    }
}
