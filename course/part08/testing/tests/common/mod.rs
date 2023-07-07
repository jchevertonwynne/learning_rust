use mongodb::options::{DropDatabaseOptions, WriteConcern};
use std::future::Future;
use testing::config::AppConfig;

pub async fn setup() -> (mongodb::Client, AppConfig, impl Future<Output = ()>) {
    let (config, mongo) = load_mongo_and_config().await;

    _setup(mongo, config)
}

fn _setup(
    mongo: mongodb::Client,
    mut config: AppConfig,
) -> (mongodb::Client, AppConfig, impl Future<Output = ()>) {
    config.mongo_config.database_info.database = format!(
        "{}-test-{}",
        config.mongo_config.database_info.database,
        uuid::Uuid::new_v4()
    );

    let cleanup = cleanup(
        mongo.clone(),
        config.mongo_config.database_info.database.clone(),
    );

    (mongo, config, cleanup)
}

async fn cleanup(mongo: mongodb::Client, database: String) {
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

async fn load_mongo_and_config() -> (AppConfig, mongodb::Client) {
    let config = AppConfig::load_from_dir("../../../config.toml").expect("failed to load config");

    let mongo = mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str())
        .await
        .expect("failed to create mongo client");

    (config, mongo)
}

#[derive(Clone)]
pub struct GlobalRuntime {
    rt: &'static tokio::runtime::Runtime,
}

pub fn rt() -> GlobalRuntime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

    let rt = RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build runtime")
    });

    GlobalRuntime { rt }
}

impl GlobalRuntime {
    pub fn block_on<F: Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }

    pub async fn setup(&self) -> (mongodb::Client, AppConfig, impl Future<Output = ()>) {
        static SETUP: tokio::sync::OnceCell<(AppConfig, mongodb::Client)> =
            tokio::sync::OnceCell::const_new();

        let (config, mongo) = SETUP.get_or_init(load_mongo_and_config).await.clone();

        _setup(mongo, config)
    }
}
