use config::ConfigError;
use mongodb::options::ConnectionString;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub mongo: ConnectionString,
    pub deck_of_cards: Url,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config.toml"))
            .build()?;
        config.try_deserialize()
    }
}
