use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::model::DeckID;

#[derive(Serialize, Deserialize)]
pub struct InteractionRecord {
    deck_id: DeckID,
    count: usize,
}

pub struct MongoRecordController {
    interactions: mongodb::Collection<InteractionRecord>,
}

impl MongoRecordController {
    pub fn new(client: &mongodb::Client) -> Self {
        let interactions = client
            .database("tracing_showcase")
            .collection("interactions");
        Self { interactions }
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
