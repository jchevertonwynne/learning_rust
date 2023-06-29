use std::future::Future;

use mongodb::options::{DropDatabaseOptions, WriteConcern};
use testing::config::AppConfig;

pub async fn setup() -> (mongodb::Client, AppConfig, impl Future<Output = ()>) {
    let config = {
        let mut config =
            AppConfig::load_from_dir("../../../config.toml").expect("failed to load config");

        config.mongo_config.database_info.database = format!(
            "{}-test-{}",
            config.mongo_config.database_info.database,
            uuid::Uuid::new_v4()
        );

        config
    };

    let mongo = mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str())
        .await
        .expect("failed to create mongo client");

    let cleanup = {
        let database = config.mongo_config.database_info.database.clone();
        let mongo = mongo.clone();
        async move {
            mongo
                .database(database.as_str())
                .drop(
                    DropDatabaseOptions::builder()
                        .write_concern(Some(WriteConcern::MAJORITY))
                        .build(),
                )
                .await
                .unwrap_or_else(|err| panic!("failed to delete database {database:?}: {err}"));
        }
    };

    (mongo, config, cleanup)
}
