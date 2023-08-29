use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    layer::SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
    EnvFilter,
    Registry,
};

#[cfg(feature = "console")]
pub fn init() -> Result<(), TryInitError> {
    use tracing_subscriber::Layer;
    Registry::default()
        .with(
            tracing_subscriber::fmt::layer().with_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            ),
        )
        .with(console_subscriber::spawn())
        .try_init()?;

    Ok(())
}

#[cfg(not(feature = "console"))]
pub fn init() -> Result<(), TryInitError> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    Ok(())
}
