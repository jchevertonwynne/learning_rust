use futures::{StreamExt, TryStreamExt};
use hyper::Body;

use tonic::codegen::Service;
use tracing::{info, instrument};

use crate::{
    deck_of_cards::{ApiError, DeckOfCardsClient},
    grpc::proto,
    model::{DeckID, DeckInfo, DrawnCardsInfo},
    mongo::MongoRecordController,
};

pub struct CardsServiceState<C>
where
    C: Service<http::Request<Body>>,
{
    cards_client: DeckOfCardsClient<C>,
    record_controller: MongoRecordController,
}

impl<C> CardsServiceState<C>
where
    C: Service<http::Request<Body>, Response = http::Response<Body>, Error = hyper::Error>
        + Send
        + Sync
        + 'static,
    C::Future: Send,
{
    pub(crate) fn new(
        cards_client: DeckOfCardsClient<C>,
        record_controller: MongoRecordController,
    ) -> Self {
        Self {
            cards_client,
            record_controller,
        }
    }

    #[instrument(skip(self))]
    pub async fn new_deck(
        &self,
        new_decks_request: NewDecksRequest,
    ) -> Result<NewDecksResponse, NewDeckError> {
        let NewDecksRequest { decks } = new_decks_request;

        let DeckInfo { deck_id, .. } = self.cards_client.new_deck(decks).await?;

        info!("created a new deck");

        self.record_controller.create(deck_id).await?;

        info!("stored deck in mongo");

        Ok(NewDecksResponse { deck_id })
    }

    #[instrument(skip(self))]
    pub async fn draw_cards(
        &self,
        draw_cards_request: DrawCardsRequest,
    ) -> Result<DrawCardsResponse, DrawCardsError> {
        let DrawCardsRequest {
            deck_id,
            hands,
            count,
        } = draw_cards_request;

        let hands = self.draw_all_cards(deck_id, hands, count).await?;

        info!("drawn all cards");

        self.record_controller.increment_count(deck_id).await?;

        info!("incremented count in mongo");

        Ok(DrawCardsResponse { hands })
    }

    #[instrument(skip(self))]
    pub async fn draw_all_cards(
        &self,
        deck_id: DeckID,
        hands: usize,
        count: u8,
    ) -> Result<Vec<DrawnCardsInfo>, ApiError> {
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
    ReqwestError(#[from] ApiError),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DrawCardsError {
    #[error("failed to draw cards: {0}")]
    ReqwestError(#[from] ApiError),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug)]
pub struct NewDecksRequest {
    decks: usize,
}

#[derive(Debug)]
pub struct NewDecksResponse {
    deck_id: DeckID,
}

impl From<NewDecksResponse> for proto::NewDecksResponse {
    fn from(value: NewDecksResponse) -> Self {
        let NewDecksResponse { deck_id } = value;
        Self {
            deck_id: deck_id.to_string(),
        }
    }
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

        let decks =
            usize::try_from(decks).map_err(|_| NewDecksRequestValidationError::InvalidDeckCount)?;

        // a regular if is probably better here, but this code isn't peer reviewed so i can do what i want
        (decks != 0)
            .then_some(())
            .ok_or(NewDecksRequestValidationError::InvalidDeckCount)?;

        Ok(NewDecksRequest { decks })
    }
}

#[derive(Debug)]
pub struct DrawCardsRequest {
    deck_id: DeckID,
    hands: usize,
    count: u8,
}

#[derive(Debug)]
pub struct DrawCardsResponse {
    hands: Vec<DrawnCardsInfo>,
}

impl From<DrawCardsResponse> for proto::DrawCardsResponse {
    fn from(value: DrawCardsResponse) -> Self {
        let DrawCardsResponse { hands } = value;
        proto::DrawCardsResponse {
            hands: hands
                .into_iter()
                .map(|hand| proto::Hand {
                    cards: hand.cards.iter().map(Into::into).collect(),
                })
                .collect(),
        }
    }
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

        let deck_id = DeckID::try_from(deck_id.as_str())
            .map_err(|_| DrawCardsRequestValidationError::DeckID)?;

        let count = u8::try_from(count).map_err(|_| DrawCardsRequestValidationError::Count)?;

        let hands = usize::try_from(hands).map_err(|_| DrawCardsRequestValidationError::Hands)?;

        Ok(DrawCardsRequest {
            deck_id,
            hands,
            count,
        })
    }
}
