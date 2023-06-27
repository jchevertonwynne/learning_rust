use mockall::predicate::eq;
use mongodb::options::{ClientOptions, ServerAddress};
use std::sync::Arc;

use reqwest::StatusCode;
use url::Url;
use wiremock::{matchers, ResponseTemplate};

use testing::{
    deck_of_cards::DeckOfCardsClient,
    model::{DeckID, DeckInfo},
    mongo::MongoRecordController,
    state::{CardsServiceState, MockMongo, NewDecksRequest, NewDecksResponse},
};

static ONCE: tokio::sync::OnceCell<Arc<mongodb::Client>> = tokio::sync::OnceCell::const_new();

async fn setup() -> Arc<mongodb::Client> {
    ONCE.get_or_init(|| async {
        println!("running the mongo connection setup!");
        let opts = ClientOptions::builder()
            .hosts(vec![ServerAddress::Tcp {
                host: "localhost".to_string(),
                port: Some(27017),
            }])
            .min_pool_size(Some(128))
            .build();
        Arc::new(mongodb::Client::with_options(opts).expect("failed to connect to mongo"))
    })
    .await
    .clone()
}

#[tokio::test]
async fn new_decks_success() -> anyhow::Result<()> {
    let mongo = setup().await;

    let deck_id = DeckID::random();
    let deck_info = DeckInfo {
        success: true,
        deck_id,
        shuffled: true,
        remaining: 52,
    };

    let mock_deck_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/deck/new/shuffle/"))
        .and(matchers::query_param("deck_count", "1"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(deck_info))
        .mount(&mock_deck_server)
        .await;

    let state = CardsServiceState::new(
        DeckOfCardsClient::new(
            Url::try_from(mock_deck_server.uri().as_str())?,
            reqwest::ClientBuilder::new().build()?,
        ),
        MongoRecordController::new(&mongo),
    );

    assert_eq!(
        NewDecksResponse { deck_id },
        state.new_deck(NewDecksRequest { decks: 1 }).await?
    );

    Ok(())
}

#[tokio::test]
async fn new_decks_mongo_failure() -> anyhow::Result<()> {
    let deck_id = DeckID::random();
    let deck_info = DeckInfo {
        success: true,
        deck_id,
        shuffled: true,
        remaining: 52,
    };

    let mock_deck_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/deck/new/shuffle/"))
        .and(matchers::query_param("deck_count", "1"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(deck_info))
        .mount(&mock_deck_server)
        .await;

    let mut mock_mongo = MockMongo::new();
    mock_mongo
        .expect_create()
        .with(eq(deck_id))
        .returning(|_| Err(mongodb::error::Error::custom("failed lol")))
        .once();

    let state = CardsServiceState::new(
        DeckOfCardsClient::new(
            Url::try_from(mock_deck_server.uri().as_str())?,
            reqwest::ClientBuilder::new().build()?,
        ),
        mock_mongo,
    );

    let resp = state.new_deck(NewDecksRequest { decks: 1 }).await;

    assert!(resp.is_err());
    let err = resp.unwrap_err();
    assert!(err.to_string().contains("failed to update mongo"));

    Ok(())
}
