use config::{ConfigError, Config};
use serde::Deserialize;
use url::Url;

/// Struct containing the deck of cards API url & the mongo config
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub mongo_config: MongoConfig,
    pub deck_of_cards: Url,
}

/// Struct containing the mongo connection string and the database info
#[derive(Debug, Deserialize, Clone)]
pub struct MongoConfig {
    pub connection_string: String,
    pub database_info: DatabaseConfig,
}

/// Struct containing the mongo database name & collections
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub database: String,
    pub collections: Collections,
}

/// Struct containing the names of all mongo collections
#[derive(Debug, Deserialize, Clone)]
pub struct Collections {
    pub interactions: String,
}

impl AppConfig {
    pub fn load_from_dir(src: &str) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(config::File::with_name(src))
            .build()
            .and_then(Config::try_deserialize)
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_dir("config.toml")
    }
}
