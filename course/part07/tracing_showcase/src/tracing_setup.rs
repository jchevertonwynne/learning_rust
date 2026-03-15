use opentelemetry::{trace::TraceError, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs::{self, LoggerProvider},
    metrics::{reader::DefaultAggregationSelector, MeterProvider, PeriodicReader},
    propagation::TraceContextPropagator,
    runtime,
    trace::{self, Sampler},
    Resource,
};
use std::sync::Once;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    layer::SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
    EnvFilter, Registry,
};

static TRACING: Once = Once::new();

pub fn init_tracing(service_name: &'static str) -> Result<TracingHandle, TracingSetupError> {
    TRACING.call_once(|| {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        let resource = Resource::new(vec![KeyValue::new("service.name", service_name)]);

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string())),
            )
            .with_trace_config(
                trace::config()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_resource(resource.clone()),
            )
            .install_batch(runtime::Tokio)
            .expect("failed to install tracer");

        let meter_provider = MeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(
                PeriodicReader::builder(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string()))
                        .build_metrics_exporter(
                            Box::new(DefaultAggregationSelector::new()),
                            Box::new(opentelemetry_sdk::metrics::reader::DefaultTemporalitySelector::new()),
                        )
                        .expect("failed to create metrics exporter"),
                    runtime::Tokio,
                )
                .build(),
            )
            .build();

        opentelemetry::global::set_meter_provider(meter_provider);


        let log_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string()))
            .build_log_exporter()
            .expect("failed to create log exporter");

        let logger_provider = LoggerProvider::builder()
            .with_config(
                logs::config().with_resource(resource.clone()),
            )
            .with_batch_exporter(log_exporter, runtime::Tokio)
            .build();
            
        let log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

        Registry::default()
            .with(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .with(log_layer)
            .try_init()
            .expect("failed to setup tracing");

        opentelemetry::global::set_logger_provider(logger_provider);
    });

    Ok(TracingHandle)
}

#[must_use]
pub struct TracingHandle;

impl Drop for TracingHandle {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
        opentelemetry::global::shutdown_logger_provider();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TracingSetupError {
    #[error("failed to install otlp layer: {0}")]
    TraceError(#[from] TraceError),
    #[error("failed to initialise registry: {0}")]
    TryInitError(#[from] TryInitError),
}
