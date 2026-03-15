use axum::{
    extract::{Path, Query, State},
    response::Response,
    Json,
};
use http::StatusCode;
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tracing::{info, info_span, instrument, Instrument};
use url::Url;

use opentelemetry::global;

use crate::{
    fake_deck_of_cards_api_state::FakeDeckOfCardsAPIState,
    model::{Card, Code, DeckID, DeckInfo, DrawnCardsInfo, Images, Suit, Value},
    mongo::RemoveCardsError,
};

const DECK_IMAGE_URL: &str = "https://smartbear.com/";

#[instrument(skip(app_state))]
pub async fn new_decks(
    Query(query): Query<NewDecksQuery>,
    app_state: State<FakeDeckOfCardsAPIState>,
) -> Result<Json<DeckInfo>, NewDecksError> {
    let deck_id = DeckID::random();

    info!("created a new deck id");

    let meter = global::meter("deck_of_cards_api");
    meter
        .u64_counter("deck_of_cards.decks_created")
        .init()
        .add(1, &[]);

    app_state
        .new_deck(deck_id)
        .instrument(info_span!("mongo_new_deck", %deck_id))
        .await?;

    info!("inserted into mongo");

    let count = query.deck_count.unwrap_or(1);
    let cards = {
        let base_image_url = Url::parse(DECK_IMAGE_URL).expect("hardcoded url should be valid");
        let _span = info_span!("generate_cards", deck_count = count).entered();

        let mut cards = Vec::with_capacity(count * 52);
        for _ in 0..count {
            append_new_deck(&mut cards, &base_image_url);
        }

        {
            let _span = info_span!("shuffle_cards").entered();
            cards.shuffle(&mut rand::thread_rng());
        }

        cards
    };

    let remaining = cards.len();
    let cards_len = cards.len();

    app_state
        .update_cards(deck_id, cards)
        .instrument(info_span!("mongo_update_cards", %deck_id, cards = cards_len))
        .await?;

    info!("updated mongo");

    Ok(Json(DeckInfo {
        success: true,
        deck_id,
        shuffled: true,
        remaining,
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewDecksQuery {
    deck_count: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum NewDecksError {
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
pub async fn draw_cards(
    Path(deck_id): Path<DeckID>,
    Query(query): Query<DrawCardsQuery>,
    app_state: State<FakeDeckOfCardsAPIState>,
) -> Result<Json<DrawnCardsInfo>, DrawCardsError> {
    let cards = app_state
        .remove_n_cards(deck_id, query.count)
        .instrument(info_span!("mongo_remove_cards", %deck_id, n = query.count))
        .await?;

    let meter = global::meter("deck_of_cards_api");
    meter
        .u64_counter("deck_of_cards.cards_drawn")
        .init()
        .add(cards.len() as u64, &[]);

    Ok(Json(DrawnCardsInfo {
        success: true,
        deck_id,
        cards: cards.into(),
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrawCardsQuery {
    count: usize,
}

impl From<RemoveCardsError> for DrawCardsError {
    fn from(value: RemoveCardsError) -> Self {
        match value {
            RemoveCardsError::Mongo(err) => DrawCardsError::Mongo(err),
            RemoveCardsError::InvalidDocument => DrawCardsError::InvalidDocument,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DrawCardsError {
    #[error("Failed to find document")]
    InvalidDocument,
    #[error("mongo operation failed: {0}")]
    Mongo(#[from] mongodb::error::Error),
}

impl axum::response::IntoResponse for DrawCardsError {
    fn into_response(self) -> Response {
        let code = match self {
            DrawCardsError::Mongo(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DrawCardsError::InvalidDocument => StatusCode::BAD_REQUEST,
        };

        (code, self.to_string()).into_response()
    }
}

fn append_new_deck(cards: &mut Vec<Card>, base_image: &Url) {
    let image_svg = {
        let mut image_svg = base_image.clone();
        image_svg.set_path("/svg");
        image_svg
    };
    let image_png = {
        let mut image_png = base_image.clone();
        image_png.set_path("/png");
        image_png
    };

    for suit in Suit::iter() {
        for value in Value::iter() {
            cards.push(Card {
                code: Code { value, suit },
                image: base_image.clone(),
                images: Images {
                    svg: image_svg.clone(),
                    png: image_png.clone(),
                },
                value,
                suit,
            })
        }
    }
}
