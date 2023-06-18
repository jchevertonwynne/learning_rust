use anyhow::Context;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use futures::FutureExt;
use mongodb::bson::doc;
use mongodb::{Collection, IndexModel};
use rand::seq::SliceRandom;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::time::Duration;

use strum::IntoEnumIterator;
use tracing::{info, info_span, instrument};
use tracing_showcase::deck_of_cards::{
    Card, Code, DeckID, DeckInfo, DrawnCardsInfo, Images, Suit, Value,
};
use tracing_showcase::init_tracing;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("fake deck of cards api")?;

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let app_state = AppState::new(&mongo_client);
    app_state.ready_database().await?;

    info!("connected to mongo...");

    let router = Router::new()
        .route("/api/deck/new/shuffle/", get(new_decks))
        .route("/api/deck/:deck_id/draw/", get(draw_cards))
        .with_state(app_state);

    let shutdown = tokio::signal::ctrl_c().map(|_| ());

    let addr = "127.0.0.1:25566"
        .to_socket_addrs()?
        .next()
        .context("expected an address")?;
    info!("serving on {addr}");

    let server = axum::Server::from_tcp(std::net::TcpListener::bind(addr)?)?
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown);

    server.await?;

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

#[derive(Debug, Clone)]
struct AppState {
    decks_collection: Collection<DeckEntry>,
}

impl AppState {
    fn new(client: &mongodb::Client) -> Self {
        Self {
            decks_collection: client.database("tracing_showcase").collection("decks"),
        }
    }

    #[instrument]
    async fn ready_database(&self) -> Result<(), mongodb::error::Error> {
        self.decks_collection.drop(None).await?;
        self.decks_collection
            .create_index(
                IndexModel::builder().keys(doc! {"deck_id": 1}).build(),
                None,
            )
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_deck_info(&self, deck_id: DeckID) -> Result<DeckEntry, mongodb::error::Error> {
        let cursor = self
            .decks_collection
            .find(doc! { "deck_id": deck_id.to_string() }, None)
            .await?;
        let found = cursor.deserialize_current()?;
        Ok(found)
    }

    #[instrument(skip(self))]
    async fn new_deck(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        let deck_entry = DeckEntry {
            deck_id,
            cards: vec![],
            card_count: 0,
        };
        self.decks_collection.insert_one(deck_entry, None).await?;
        Ok(())
    }

    #[instrument(skip(self, cards))]
    async fn update_cards(
        &self,
        deck_id: DeckID,
        cards: Vec<Card>,
    ) -> Result<(), mongodb::error::Error> {
        let card_count = mongodb::bson::to_bson(&cards.len())?;
        let cards = mongodb::bson::to_bson(&cards)?;
        self.decks_collection
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                doc! { "$inc": {  "card_count": card_count }, "$push": { "cards":  { "$each": cards } } },
                None,
            )
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn remove_n_cards(
        &self,
        deck_id: DeckID,
        n_cards: usize,
    ) -> Result<Vec<Card>, RemoveCardsError> {
        let n_cards_bson = mongodb::bson::to_bson(&n_cards)?;

        let DeckEntry { mut cards, .. } = self
            .decks_collection
            .find_one_and_update(
                doc! {
                    "deck_id": deck_id.to_string(),
                    "card_count": { "$gte": n_cards_bson.clone()
                    }
                },
                vec![doc! {
                    "$set": {
                        "card_count": { "$subtract": [ "$card_count", n_cards_bson.clone() ] },
                        "cards": {
                            "$slice": ["$cards", 0, { "$subtract": [ "$card_count", n_cards_bson.clone() ] } ]
                        },
                    }
                }],
                None,
            )
            .await?
            .ok_or(RemoveCardsError::InvalidDocument)?;

        let mut result = Vec::with_capacity(n_cards);
        for _ in 0..n_cards {
            result.push(cards.pop().expect("ensured there are enough cars already"))
        }
        info!("removed cards");
        Ok(result)
    }
}

#[derive(Debug, thiserror::Error)]
enum RemoveCardsError {
    #[error("Failed to find document")]
    InvalidDocument,
    #[error("mongo operation failed: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("failed to convert value to bson: {0}")]
    Bson(#[from] mongodb::bson::ser::Error),
}

#[instrument(skip(app_state))]
async fn new_decks(
    Query(query): Query<NewDecksQuery>,
    app_state: State<AppState>,
) -> Result<Json<DeckInfo>, NewDecksError> {
    let deck_id = DeckID::random();

    let span = info_span!("created deck id", %deck_id);
    let _entered = span.enter();
    info!("created a new deck id");

    app_state.new_deck(deck_id).await?;

    info!("inserted into mongo");

    let mut cards = vec![];
    for _ in 0..query.deck_count.unwrap_or(1) {
        cards.append(&mut generate_deck_of_cards());
    }
    cards.shuffle(&mut rand::thread_rng());

    let remaining = cards.len();

    app_state.update_cards(deck_id, cards).await?;

    info!("updated mongo");

    Ok(Json(DeckInfo {
        success: true,
        deck_id,
        shuffled: true,
        remaining,
    }))
}

#[derive(Debug, Serialize, Deserialize)]
struct NewDecksQuery {
    deck_count: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
enum NewDecksError {
    #[error("mongo operation failed: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

impl axum::response::IntoResponse for NewDecksError {
    fn into_response(self) -> Response {
        let code = match self {
            NewDecksError::MongoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (code, self.to_string()).into_response()
    }
}

#[instrument(skip(app_state))]
async fn draw_cards(
    Path(deck_id): Path<DeckID>,
    Query(query): Query<DrawCardsQuery>,
    app_state: State<AppState>,
) -> Result<Json<DrawnCardsInfo>, DrawCardsError> {
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cards = app_state.remove_n_cards(deck_id, query.count).await?;

    Ok(Json(DrawnCardsInfo {
        success: true,
        deck_id,
        cards: cards.into(),
    }))
}

#[derive(Debug, Serialize, Deserialize)]
struct DrawCardsQuery {
    count: usize,
}

impl From<RemoveCardsError> for DrawCardsError {
    fn from(value: RemoveCardsError) -> Self {
        match value {
            RemoveCardsError::Mongo(err) => DrawCardsError::Mongo(err),
            RemoveCardsError::InvalidDocument => DrawCardsError::InvalidDocument,
            RemoveCardsError::Bson(err) => DrawCardsError::Bson(err),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum DrawCardsError {
    #[error("Failed to find document")]
    InvalidDocument,
    #[error("mongo operation failed: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("failed to convert value to bson: {0}")]
    Bson(#[from] mongodb::bson::ser::Error),
}

impl axum::response::IntoResponse for DrawCardsError {
    fn into_response(self) -> Response {
        let code = match self {
            DrawCardsError::Mongo(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DrawCardsError::InvalidDocument => StatusCode::BAD_REQUEST,
            DrawCardsError::Bson(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (code, self.to_string()).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DeckEntry {
    deck_id: DeckID,
    cards: Vec<Card>,
    card_count: usize,
}

fn generate_deck_of_cards() -> Vec<Card> {
    let mut cards = Vec::with_capacity(52);

    let image = Url::from_str("https://smartbear.com/").expect("should be a valid url");
    let image_svg = {
        let mut image_svg = image.clone();
        image_svg.set_path("/svg");
        image_svg
    };
    let image_png = {
        let mut image_png = image.clone();
        image_png.set_path("/png");
        image_png
    };

    for suit in Suit::iter() {
        for value in Value::iter() {
            cards.push(Card {
                code: Code { value, suit },
                image: image.clone(),
                images: Images {
                    svg: image_svg.clone(),
                    png: image_png.clone(),
                },
                value,
                suit,
            })
        }
    }

    cards
}
