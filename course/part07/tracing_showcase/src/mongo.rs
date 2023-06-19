use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::model::DeckID;

#[derive(Serialize, Deserialize)]
pub struct InteractionRecord {
    deck_id: String,
    count: usize,
}

pub struct MongoRecordController {
    interactions: mongodb::Collection<InteractionRecord>,
}

impl MongoRecordController {
    pub fn new(client: &mongodb::Client) -> Self {
        let collection = client
            .database("tracing_showcase")
            .collection("interactions");
        Self {
            interactions: collection,
        }
    }

    #[instrument(skip(self))]
    pub async fn create(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("creating a new record");
        self.interactions
            .insert_one(
                InteractionRecord {
                    deck_id: deck_id.to_string(),
                    count: 0,
                },
                None,
            )
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn increment_count(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("incrementing count");
        self.interactions
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                doc! { "$inc": { "count": 1 } },
                None,
            )
            .await?;
        Ok(())
    }
}
