use reqwest::Client;
use url::Url;

use crate::model::{DeckID, DeckInfo, DrawnCardsInfo};

pub struct DeckOfCardsClient {
    base_url: Url,
    client: Client,
}

impl DeckOfCardsClient {
    pub fn new(mut base_url: Url, client: Client) -> Self {
        base_url.set_path("/");
        Self { base_url, client }
    }

    #[tracing::instrument(skip(self))]
    pub async fn new_deck(&self, decks: usize) -> Result<DeckInfo, reqwest::Error> {
        let mut url = self.base_url.clone();
        url.set_path("/api/deck/new/shuffle/");
        url.set_query(Some(&format!("deck_count={decks}")));
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn draw_cards(
        &self,
        deck_id: DeckID,
        n: u8,
    ) -> Result<DrawnCardsInfo, reqwest::Error> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("/api/deck/{deck_id}/draw/"));
        url.set_query(Some(&format!("count={n}")));
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}
