use crate::model::{Card, DeckID};
use crate::mongo::{MongoRecordController, RemoveCardsError};
use tracing::instrument;

#[derive(Clone)]
pub struct DeckOfCardsAPIState {
    controller: MongoRecordController,
}

impl DeckOfCardsAPIState {
    pub fn new(client: &mongodb::Client) -> Self {
        Self {
            controller: MongoRecordController::new(client),
        }
    }

    #[instrument(skip(self))]
    pub async fn new_deck(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        self.controller.new_deck(deck_id).await
    }

    #[instrument(skip(self, cards))]
    pub async fn update_cards(
        &self,
        deck_id: DeckID,
        cards: Vec<Card>,
    ) -> Result<(), mongodb::error::Error> {
        self.controller.insert_cards(deck_id, cards).await
    }

    #[instrument(skip(self))]
    pub async fn remove_n_cards(
        &self,
        deck_id: DeckID,
        n_cards: usize,
    ) -> Result<Vec<Card>, RemoveCardsError> {
        self.controller.remove_n_cards(deck_id, n_cards).await
    }
}
