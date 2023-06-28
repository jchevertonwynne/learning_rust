use async_trait::async_trait;
use url::Url;

use crate::model::{DeckID, DeckInfo, DrawnCardsInfo};

pub struct DeckOfCardsClient {
    base_url: Url,
    client: reqwest::Client,
}

impl DeckOfCardsClient {
    pub fn new(mut base_url: Url, client: reqwest::Client) -> Self {
        base_url.set_path("/");
        Self { base_url, client }
    }
}

#[async_trait]
impl crate::state::DeckOfCards for DeckOfCardsClient {
    async fn new_deck(&self, decks: usize) -> Result<DeckInfo, reqwest::Error> {
        let mut url = self.base_url.clone();
        url.set_path("/api/deck/new/shuffle/");
        url.set_query(Some(&format!("deck_count={decks}")));
        let res: DeckInfo = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(res)
    }

    async fn draw_cards(&self, deck_id: DeckID, n: u8) -> Result<DrawnCardsInfo, reqwest::Error> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("/api/deck/{deck_id}/draw/"));
        url.set_query(Some(&format!("count={n}")));
        let res: DrawnCardsInfo = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use crate::state::DeckOfCards;
    use reqwest::StatusCode;
    use wiremock::{matchers, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn get_new_deck_success() -> anyhow::Result<()> {
        let mock_server = wiremock::MockServer::start().await;

        let deck_info = DeckInfo {
            success: true,
            deck_id: DeckID::random(),
            shuffled: true,
            remaining: 52,
        };

        wiremock::Mock::given(matchers::method("GET"))
            .and(matchers::path("/api/deck/new/shuffle/"))
            .and(matchers::query_param("deck_count", "1"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(deck_info.clone()))
            .mount(&mock_server)
            .await;

        let mock_server_url = Url::try_from(mock_server.uri().as_str())?;
        let request_client = reqwest::ClientBuilder::new().build()?;
        let deck_client = DeckOfCardsClient::new(mock_server_url, request_client);

        let returned_deck_info = deck_client
            .new_deck(1)
            .await
            .expect("expected request to succeed");
        assert_eq!(deck_info, returned_deck_info);

        Ok(())
    }

    #[tokio::test]
    async fn get_new_deck_failure_on_non_200_code() -> anyhow::Result<()> {
        let mock_server = wiremock::MockServer::start().await;

        let deck_info = DeckInfo {
            success: true,
            deck_id: DeckID::random(),
            shuffled: true,
            remaining: 52,
        };

        wiremock::Mock::given(matchers::method("GET"))
            .and(matchers::path("/api/deck/new/shuffle/"))
            .and(matchers::query_param("deck_count", "1"))
            .respond_with(
                ResponseTemplate::new(StatusCode::BAD_REQUEST).set_body_json(deck_info.clone()),
            )
            .mount(&mock_server)
            .await;

        let mock_server_url = Url::try_from(mock_server.uri().as_str())?;
        let request_client = reqwest::ClientBuilder::new().build()?;
        let deck_client = DeckOfCardsClient::new(mock_server_url, request_client);

        let new_deck_response = deck_client.new_deck(1).await;
        assert!(
            matches!(new_deck_response, Err(_)),
            "expected an error for a non-200 response code"
        );

        println!("{:?}", new_deck_response);

        Ok(())
    }
}
