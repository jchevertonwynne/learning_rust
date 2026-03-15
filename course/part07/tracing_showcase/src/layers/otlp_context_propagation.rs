use std::{
    task::{Context, Poll},
};

use tower::{Layer, Service};
use tracing::{info_span, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Clone, Default)]
pub struct OtlpPropagatedTracingContextConsumerLayer;

impl OtlpPropagatedTracingContextConsumerLayer {
    pub fn new() -> Self {
        OtlpPropagatedTracingContextConsumerLayer
    }
}

impl<S> Layer<S> for OtlpPropagatedTracingContextConsumerLayer {
    type Service = OtlpPropagatedTracingContextConsumerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OtlpPropagatedTracingContextConsumerService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct OtlpPropagatedTracingContextConsumerService<S> {
    inner: S,
}

impl<S, I, O> Service<http::Request<I>> for OtlpPropagatedTracingContextConsumerService<S>
where
    S: Service<http::Request<I>, Response = O>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Instrumented<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<I>) -> Self::Future {
        let parent_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&opentelemetry_http::HeaderExtractor(req.headers()))
        });
        let span = info_span!("handling a request", uri = %req.uri());
        span.set_parent(parent_ctx);
        Instrument::instrument(self.inner.call(req), span)
    }
}

#[derive(Debug, Clone, Default)]
pub struct OtlpPropagatedTracingContextProducerLayer;

impl<S> Layer<S> for OtlpPropagatedTracingContextProducerLayer {
    type Service = OtlpPropagatedTracingContextProducerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OtlpPropagatedTracingContextProducerService { inner }
    }
}

pub struct OtlpPropagatedTracingContextProducerService<S> {
    inner: S,
}

impl<S, I> Service<http::Request<I>> for OtlpPropagatedTracingContextProducerService<S>
where
    S: Service<http::Request<I>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<I>) -> Self::Future {
        let ctx = tracing::Span::current().context();
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&ctx, &mut opentelemetry_http::HeaderInjector(req.headers_mut()));
        });
        self.inner.call(req)
    }
}
