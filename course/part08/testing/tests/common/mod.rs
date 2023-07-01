use mongodb::options::{DropDatabaseOptions, WriteConcern};
use std::future::Future;
use testing::config::AppConfig;

static SETUP_ONCE: tokio::sync::OnceCell<(AppConfig, mongodb::Client)> =
    tokio::sync::OnceCell::const_new();

pub async fn setup() -> anyhow::Result<(
    mongodb::Client,
    AppConfig,
    impl Future<Output = anyhow::Result<()>>,
)> {
    let (mut config, mongo) = SETUP_ONCE
        .get_or_init(|| async {
            let config =
                AppConfig::load_from_dir("../../../config.toml").expect("failed to load config");

            let mongo =
                mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str())
                    .await
                    .expect("failed to create mongo client");

            (config, mongo)
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
