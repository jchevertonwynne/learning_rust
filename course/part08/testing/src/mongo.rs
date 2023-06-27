use async_trait::async_trait;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};

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
    interactions: mongodb::Collection<InteractionRecord>,
}

#[async_trait]
impl crate::state::Mongo for MongoRecordController {
    async fn create(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        MongoRecordController::create(self, deck_id).await
    }

    async fn increment_count(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        MongoRecordController::increment_count(self, deck_id).await
    }
}

impl MongoRecordController {
    pub fn new(client: &mongodb::Client) -> Self {
        let db = client.database("tracing_showcase");
        let interactions = db.collection("interactions");
        Self { interactions }
    }

    pub async fn create(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        self.interactions
            .insert_one(InteractionRecord { deck_id, count: 0 }, None)
            .await?;

        Ok(())
    }

    pub async fn increment_count(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
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
