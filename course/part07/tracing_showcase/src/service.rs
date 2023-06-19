use futures::{StreamExt, TryStreamExt};
use tracing::info;

use crate::{deck_of_cards::DeckOfCardsClient, mongo::MongoRecordController, model::{DeckID, DeckInfo, DrawnCardsInfo}, grpc::proto};

pub struct CardsServiceInternal {
    cards_client: DeckOfCardsClient,
    record_controller: MongoRecordController,
}

impl CardsServiceInternal {
    pub fn new(cards_client: DeckOfCardsClient, record_controller: MongoRecordController) -> Self {
        Self {
            cards_client,
            record_controller,
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn new_deck(
        &self,
        new_decks_request: NewDecksRequest,
    ) -> Result<DeckID, NewDeckError> {
        let NewDecksRequest { decks } = new_decks_request;

        let DeckInfo { deck_id, .. } = self.cards_client.new_deck(decks).await?;

        info!("created a new deck");

        self.record_controller.create(deck_id).await?;

        info!("stored deck in mongo");

        Ok(deck_id)
    }

    #[tracing::instrument(skip(self))]
    pub async fn draw_cards(
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
    pub async fn draw_all_cards(
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

#[derive(Debug, thiserror::Error)]
pub enum NewDeckError {
    #[error("failed to draw deck: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DrawCardsError {
    #[error("failed to draw cards: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug)]
pub struct NewDecksRequest {
    decks: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum NewDecksRequestValidationError {
    #[error("count must be a positive integer")]
    InvalidDeckCount,
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<proto::NewDecksRequest> for NewDecksRequest {
    type Error = NewDecksRequestValidationError;

    fn try_from(value: proto::NewDecksRequest) -> Result<Self, Self::Error> {
        let proto::NewDecksRequest { decks } = value;

        let Ok(decks) = usize::try_from(decks) else {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        };

        if decks == 0 {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        }

        Ok(NewDecksRequest { decks })
    }
}

#[derive(Debug)]
pub struct DrawCardsRequest {
    deck_id: DeckID,
    hands: usize,
    count: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum DrawCardsRequestValidationError {
    #[error("a deck id must be 12 lowercase letters and numbers")]
    DeckID,
    #[error("hands must be a positive integer")]
    Hands,
    #[error("count must be a positive u8 value")]
    Count,
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<proto::DrawCardsRequest> for DrawCardsRequest {
    type Error = DrawCardsRequestValidationError;

    fn try_from(value: proto::DrawCardsRequest) -> Result<Self, Self::Error> {
        let proto::DrawCardsRequest {
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