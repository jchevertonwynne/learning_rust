use axum::{
    extract::{Path, Query, State},
    response::Response,
    Json,
};
use http::StatusCode;
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use std::{str::FromStr, time::Duration};
use strum::IntoEnumIterator;
use tracing::{info, instrument};
use url::Url;

use crate::{
    fake_deck_of_cards_api_state::FakeDeckOfCardsAPIState,
    model::{Card, Code, DeckID, DeckInfo, DrawnCardsInfo, Images, Suit, Value},
    mongo::RemoveCardsError,
};

#[instrument(skip(app_state))]
pub async fn new_decks(
    Query(query): Query<NewDecksQuery>,
    app_state: State<FakeDeckOfCardsAPIState>,
) -> Result<Json<DeckInfo>, NewDecksError> {
    let deck_id = DeckID::random();

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
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cards = app_state.remove_n_cards(deck_id, query.count).await?;

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
