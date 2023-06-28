use std::future::Future;
use std::sync::Arc;

use mongodb::options::{DropDatabaseOptions, WriteConcern};
use testing::config::AppConfig;

static ONCE: tokio::sync::OnceCell<(Arc<mongodb::Client>, AppConfig)> = tokio::sync::OnceCell::const_new();

pub async fn setup() -> (Arc<mongodb::Client>, AppConfig, impl Future<Output = ()>) {
    let (mongo, mut config) = ONCE
        .get_or_init(|| async {
            let config =
                AppConfig::load_from_dir("../../../config.toml").expect("failed to load config");

            let mongo_client = mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str())
                .await
                .expect("failed to parse connection string");

            (Arc::new(mongo_client), config)
        })
        .await
        .clone();

    config.mongo_config.database_info.database = format!(
        "{}-test-{}",
        config.mongo_config.database_info.database,
        uuid::Uuid::new_v4()
    );

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
                .expect(&format!("failed to delete database {database:?}"));
        }
    };

    (mongo, config, cleanup)
}