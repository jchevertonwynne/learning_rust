mod common;

use mockall::predicate::eq;
use reqwest::StatusCode;
use url::Url;
use wiremock::{matchers, ResponseTemplate};

use testing::{
    deck_of_cards::DeckOfCardsClient,
    model::{DeckID, DeckInfo},
    mongo::MongoRecordController,
    state::{DeckService, MockMongo, NewDeckError, NewDecksRequest, NewDecksResponse},
};
use testing_cleanup::test_with_cleanup;

#[tokio::test]
async fn new_decks_success_flawed_cleanup() -> anyhow::Result<()> {
    let (mongo, config, cleanup) = common::setup().await;

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

    let state = DeckService::new(
        DeckOfCardsClient::new(
            Url::try_from(mock_deck_server.uri().as_str())?,
            reqwest::ClientBuilder::new().build()?,
        ),
        MongoRecordController::new(&mongo, config.mongo_config.database_info),
    );

    assert_eq!(
        NewDecksResponse { deck_id },
        state.new_deck(NewDecksRequest { decks: 1 }).await?,
        "expected a response with the predetermined deck ID"
    );

    cleanup.await;

    Ok(())
}

#[tokio::test]
async fn new_decks_success_manual_cleanup() -> anyhow::Result<()> {
    let (mongo, config, cleanup) = common::setup().await;

    let handle = tokio::spawn(async move {
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

        let state = DeckService::new(
            DeckOfCardsClient::new(
                Url::try_from(mock_deck_server.uri().as_str())?,
                reqwest::ClientBuilder::new().build()?,
            ),
            MongoRecordController::new(&mongo, config.mongo_config.database_info),
        );

        assert_eq!(
            NewDecksResponse { deck_id },
            state.new_deck(NewDecksRequest { decks: 1 }).await?,
            "expected a response with the predetermined deck ID"
        );

        Ok::<(), anyhow::Error>(())
    });

    let res = handle.await;

    cleanup.await;

    res??;

    Ok(())
}

#[test_with_cleanup]
async fn my_test_automatic_cleanup(
    mongo: mongodb::Client,
    config: testing::config::AppConfig,
) -> anyhow::Result<()> {
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

    let state = DeckService::new(
        DeckOfCardsClient::new(
            Url::try_from(mock_deck_server.uri().as_str())?,
            reqwest::ClientBuilder::new().build()?,
        ),
        MongoRecordController::new(&mongo, config.mongo_config.database_info),
    );

    assert_eq!(
        NewDecksResponse { deck_id },
        state.new_deck(NewDecksRequest { decks: 1 }).await?,
        "expected a response with the predetermined deck ID"
    );

    Ok::<(), anyhow::Error>(())
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

    let state = DeckService::new(
        DeckOfCardsClient::new(
            Url::try_from(mock_deck_server.uri().as_str())?,
            reqwest::ClientBuilder::new().build()?,
        ),
        mock_mongo,
    );

    let resp = state.new_deck(NewDecksRequest { decks: 1 }).await;

    assert!(
        matches!(resp, Err(NewDeckError::MongoError(_))),
        "expected a mongo call error"
    );
    assert!(
        resp.unwrap_err()
            .to_string()
            .contains("failed to update mongo"),
        "error string did not contain the expected contents"
    );

    Ok(())
}
