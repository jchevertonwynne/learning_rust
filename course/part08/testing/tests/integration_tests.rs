use mockall::predicate::eq;
use mongodb::{
    bson::uuid,
    options::{DropDatabaseOptions, WriteConcern},
    Client,
};
use std::{future::Future, sync::Arc};

use reqwest::StatusCode;
use url::Url;
use wiremock::{matchers, ResponseTemplate};

use testing::{
    config::AppConfig,
    deck_of_cards::DeckOfCardsClient,
    model::{DeckID, DeckInfo},
    mongo::MongoRecordController,
    state::{CardsServiceState, MockMongo, NewDecksRequest, NewDecksResponse},
};

static ONCE: tokio::sync::OnceCell<Arc<Client>> = tokio::sync::OnceCell::const_new();

async fn setup() -> (Arc<Client>, AppConfig, impl Future<Output = ()>) {
    let mut config = AppConfig::load_from_dir("../../../config.toml").unwrap();
    config.mongo_config.database_info.database = format!(
        "{}-test-{}",
        config.mongo_config.database_info.database,
        uuid::Uuid::new()
    );

    let mongo = ONCE
        .get_or_init(|| async {
            let mongo_client = Client::with_uri_str(config.mongo_config.connection_string.as_str())
                .await
                .expect("failed to parse connection string");
            Arc::new(mongo_client)
        })
        .await
        .clone();

    let cleanup = {
        let database = config.mongo_config.database_info.database.clone();
        let mongo = Arc::clone(&mongo);
        async move {
            mongo
                .database(database.as_str())
                .drop(
                    DropDatabaseOptions::builder()
                        .write_concern(Some(WriteConcern::MAJORITY))
                        .build(),
                )
                .await
                .expect("failed to delete database");
        }
    };

    (mongo, config, cleanup)
}

#[tokio::test]
async fn new_decks_success() -> anyhow::Result<()> {
    let (mongo, config, cleanup) = setup().await;

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

        let state = CardsServiceState::new(
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

    assert!(resp.is_err(), "expected call to return an error");
    let err = resp.unwrap_err();
    assert!(
        err.to_string().contains("failed to update mongo"),
        "error string did not contain the expected contents"
    );

    Ok(())
}
