use opentelemetry::trace::TraceError;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};
use tracing_subscriber::{EnvFilter, Registry};

pub fn init_tracing(service_name: &str) -> Result<(), TracingSetupError> {
    opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_opentelemetry::layer().with_tracer(
                opentelemetry_jaeger::new_agent_pipeline()
                    .with_service_name(service_name)
                    .with_max_packet_size(8192)
                    .with_auto_split_batch(true)
                    .install_batch(opentelemetry::runtime::Tokio)?,
            ),
        )
        .try_init()?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum TracingSetupError {
    #[error("failed to install jaeger layer: {0}")]
    TraceError(#[from] TraceError),
    #[error("failed to initialise registry: {0}")]
    TryInitError(#[from] TryInitError),
}