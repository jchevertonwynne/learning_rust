use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::model::{Card, DeckID};

#[derive(Serialize, Deserialize)]
pub struct InteractionRecord {
    deck_id: DeckID,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeckEntry {
    deck_id: DeckID,
    cards: Vec<Card>,
    card_count: usize,
}

#[derive(Clone)]
pub struct MongoRecordController {
    entries: mongodb::Collection<DeckEntry>,
    interactions: mongodb::Collection<InteractionRecord>,
}

impl MongoRecordController {
    pub fn new(client: &mongodb::Client) -> Self {
        let db = client.database("tracing_showcase");
        let entries = db.collection("entries");
        let interactions = db.collection("interactions");
        Self {
            entries,
            interactions,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_deck_info(&self, deck_id: DeckID) -> Result<DeckEntry, mongodb::error::Error> {
        let cursor = self
            .entries
            .find(doc! { "deck_id": deck_id.to_string() }, None)
            .await?;
        let found = cursor.deserialize_current()?;
        Ok(found)
    }

    #[instrument(skip(self))]
    pub async fn new_deck(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        let deck_entry = DeckEntry {
            deck_id,
            cards: vec![],
            card_count: 0,
        };
        self.entries.insert_one(deck_entry, None).await?;
        Ok(())
    }

    #[instrument(skip(self, cards))]
    pub async fn insert_cards(
        &self,
        deck_id: DeckID,
        cards: Vec<Card>,
    ) -> Result<(), mongodb::error::Error> {
        self.entries
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                doc! {
                    "$inc": {
                        "card_count": cards.len() as i64
                    },
                    "$push": {
                        "cards":  { "$each": cards }
                    }
                },
                None,
            )
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn remove_n_cards(
        &self,
        deck_id: DeckID,
        n_cards: usize,
    ) -> Result<Vec<Card>, RemoveCardsError> {
        let DeckEntry { mut cards, .. } = self
            .entries
            .find_one_and_update(
                doc! {
                    "deck_id": deck_id.to_string(),
                    "card_count": { "$gte": n_cards as i64 }
                },
                vec![doc! {
                    "$set": {
                        "card_count": { "$subtract": [ "$card_count", n_cards as i64 ] },
                        "cards": {
                            "$slice": ["$cards", 0, { "$subtract": [ "$card_count", n_cards as i64 ] } ]
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

    #[instrument(skip(self))]
    pub async fn create(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        info!("creating a new record");

        self.interactions
            .insert_one(InteractionRecord { deck_id, count: 0 }, None)
            .await?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn increment_count(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        info!("incrementing count");

        self.interactions
            .update_one(
                doc! { "deck_id": deck_id },
                doc! { "$inc": { "count": 1 } },
                None,
            )
            .await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveCardsError {
    #[error("Failed to find document")]
    InvalidDocument,
    #[error("mongo operation failed: {0}")]
    Mongo(#[from] mongodb::error::Error),
}
