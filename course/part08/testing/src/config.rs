use config::ConfigError;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub mongo_config: MongoConfig,
    pub deck_of_cards: Url,
}

#[derive(Debug, Deserialize)]
pub struct MongoConfig {
    pub connection_string: String,
    pub database_info: DatabaseConfig,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub database: String,
    pub collections: Collections,
}

#[derive(Debug, Deserialize)]
pub struct Collections {
    pub interactions: String,
}

impl AppConfig {
    pub fn load_from_dir(src: &str) -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name(src))
            .build()?;
        config.try_deserialize()
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_dir("config.toml")
    }
}
