use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use mockall::automock;

use crate::model::{DeckID, DeckInfo, DrawnCardsInfo};

#[automock]
#[async_trait]
pub trait DeckOfCards {
    async fn new_deck(&self, decks: usize) -> Result<DeckInfo, reqwest::Error>;
    async fn draw_cards(&self, deck_id: DeckID, n: u8) -> Result<DrawnCardsInfo, reqwest::Error>;
}

#[automock]
#[async_trait]
pub trait Mongo {
    async fn create(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error>;
    async fn increment_count(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error>;
}

pub struct DeckService<D, M> {
    deck_client: D,
    record_controller: M,
}

impl<D, M> DeckService<D, M> {
    pub fn new(deck_client: D, record_controller: M) -> Self {
        Self {
            deck_client,
            record_controller,
        }
    }
}

impl<D, M> DeckService<D, M>
where
    D: DeckOfCards,
    M: Mongo,
{
    pub async fn new_deck(
        &self,
        new_decks_request: NewDecksRequest,
    ) -> Result<NewDecksResponse, NewDeckError> {
        let NewDecksRequest { decks } = new_decks_request;

        let DeckInfo {
            deck_id, success, ..
        } = self.deck_client.new_deck(decks).await?;

        if !success {
            return Err(NewDeckError::FalseSuccess);
        }

        self.record_controller.create(deck_id).await?;

        Ok(NewDecksResponse { deck_id })
    }

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

        if hands.iter().any(|h| !h.success) {
            return Err(DrawCardsError::FalseSuccess);
        }

        self.record_controller.increment_count(deck_id).await?;

        Ok(DrawCardsResponse { hands })
    }
}

impl<D, M> DeckService<D, M>
where
    D: DeckOfCards,
{
    async fn draw_all_cards(
        &self,
        deck_id: DeckID,
        hands: usize,
        count: u8,
    ) -> Result<Vec<DrawnCardsInfo>, reqwest::Error> {
        futures::stream::iter((0..hands).map(|_| self.deck_client.draw_cards(deck_id, count)))
            .buffer_unordered(5)
            .try_collect()
            .await
    }
}

#[derive(Debug)]
pub struct NewDecksRequest {
    pub decks: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NewDecksResponse {
    pub deck_id: DeckID,
}

#[derive(Debug, thiserror::Error)]
pub enum NewDeckError {
    #[error("request did not return back success value")]
    FalseSuccess,
    #[error("failed to request new deck: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug)]
pub struct DrawCardsRequest {
    pub deck_id: DeckID,
    pub hands: usize,
    pub count: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DrawCardsResponse {
    pub hands: Vec<DrawnCardsInfo>,
}

#[derive(Debug, thiserror::Error)]
pub enum DrawCardsError {
    #[error("request did not return back success value")]
    FalseSuccess,
    #[error("failed to request drawn cards: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[cfg(test)]
mod tests {
    use mockall::predicate::eq;

    use super::*;

    #[tokio::test]
    async fn my_int_test_success() -> anyhow::Result<()> {
        let mut mock_deck_of_cards = MockDeckOfCards::new();
        let mut mock_mongo = MockMongo::new();
        let mut sequence = mockall::Sequence::new();

        let deck_id = DeckID::random();

        mock_deck_of_cards
            .expect_new_deck()
            .with(eq(1))
            .returning(move |input| {
                Ok(DeckInfo {
                    success: true,
                    deck_id,
                    shuffled: true,
                    remaining: input * 52,
                })
            })
            .once()
            .in_sequence(&mut sequence);

        mock_mongo
            .expect_create()
            .with(eq(deck_id))
            .returning(|_| Ok(()))
            .once()
            .in_sequence(&mut sequence);

        let state = DeckService::new(mock_deck_of_cards, mock_mongo);

        assert_eq!(
            NewDecksResponse { deck_id },
            state.new_deck(NewDecksRequest { decks: 1 }).await?
        );

        Ok(())
    }

    #[tokio::test]
    async fn my_int_test_fail_on_not_success_response() -> anyhow::Result<()> {
        let mut mock_deck_of_cards = MockDeckOfCards::new();
        let mock_mongo = MockMongo::new();

        let deck_id = DeckID::random();

        mock_deck_of_cards
            .expect_new_deck()
            .with(eq(1))
            .returning(move |_| {
                Ok(DeckInfo {
                    success: false,
                    deck_id,
                    shuffled: true,
                    remaining: 0,
                })
            })
            .once();

        let state = DeckService::new(mock_deck_of_cards, mock_mongo);

        assert!(state.new_deck(NewDecksRequest { decks: 1 }).await.is_err());

        Ok(())
    }
}
