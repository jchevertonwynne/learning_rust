use std::future::Future;
use anyhow::{Context};

use mongodb::options::{DropDatabaseOptions, WriteConcern};
use testing::config::AppConfig;

pub async fn setup() -> anyhow::Result<(mongodb::Client, AppConfig, impl Future<Output = anyhow::Result<()>>)> {
    let config = {
        let mut config =
            AppConfig::load_from_dir("../../../config.toml").context("failed to load config")?;

        config.mongo_config.database_info.database = format!(
            "{}-test-{}",
            config.mongo_config.database_info.database,
            uuid::Uuid::new_v4()
        );

        config
    };

    let mongo = mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str())
        .await
        .context("failed to create mongo client")?;

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
                .map_err(|err| anyhow::anyhow!("failed to delete database {database:?}: {err}"))?;

            Ok(())
        }
    };

    Ok((mongo, config, cleanup))
}
