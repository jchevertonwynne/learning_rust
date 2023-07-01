use async_trait::async_trait;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};

use crate::{
    config::DatabaseConfig,
    model::{Card, DeckID},
};

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

impl MongoRecordController {
    pub fn new(client: &mongodb::Client, config: DatabaseConfig) -> Self {
        let db = client.database(config.database.as_str());
        let interactions = db.collection(config.collections.interactions.as_str());
        Self { interactions }
    }
}

#[async_trait]
impl crate::state::Mongo for MongoRecordController {
    async fn create(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
        self.interactions
            .insert_one(InteractionRecord { deck_id, count: 0 }, None)
            .await?;

        Ok(())
    }

    async fn increment_count(&self, deck_id: DeckID) -> Result<(), mongodb::error::Error> {
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
