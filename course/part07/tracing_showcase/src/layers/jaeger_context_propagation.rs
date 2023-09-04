use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    task::{Context, Poll},
};

use fxhash::FxBuildHasher;
use http::{HeaderName, HeaderValue};

use tower::{Layer, Service};
use tracing::{error, info, info_span, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Clone, Default)]
pub struct JaegerPropagatedTracingContextConsumerLayer;

impl JaegerPropagatedTracingContextConsumerLayer {
    pub fn new() -> Self {
        JaegerPropagatedTracingContextConsumerLayer
    }
}

impl<S> Layer<S> for JaegerPropagatedTracingContextConsumerLayer {
    type Service = JaegerPropagatedTracingContextConsumerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JaegerPropagatedTracingContextConsumerService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct JaegerPropagatedTracingContextConsumerService<S> {
    inner: S,
}

impl<S, I, O> Service<http::Request<I>> for JaegerPropagatedTracingContextConsumerService<S>
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
        std::thread_local! {
            static PARENT_CTX_MAP: RefCell<HashMap<String, String, FxBuildHasher>> = RefCell::new(HashMap::with_hasher(FxBuildHasher::default()));
        }

        PARENT_CTX_MAP.with(|parent_ctx_map| {
            let mut parent_ctx_map = parent_ctx_map.borrow_mut();
            parent_ctx_map.clear();

            // let mut parent_ctx_map = HashMap::<String, String>::new();
            for (k, v) in req.headers() {
                let k = k.as_str();
                if k == "uber-trace-id" || k.starts_with("uberctx-") {
                    let k = k.to_string();
                    let v = match v.to_str() {
                        Ok(v) => v,
                        Err(err) => {
                            error!("failed to convert value for header {k:?}:{v:?}: {err}");
                            continue;
                        }
                    }
                    .to_string();
                    parent_ctx_map.insert(k, v);
                }
            }

            let span = info_span!("handling a request", uri = %req.uri());

            let parent_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.extract(parent_ctx_map.deref())
            });

            span.set_parent(parent_ctx);
            Instrument::instrument(self.inner.call(req), span)
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct JaegerPropagatedTracingContextProducerLayer;

impl<S> Layer<S> for JaegerPropagatedTracingContextProducerLayer {
    type Service = JaegerPropagatedTracingContextProducerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JaegerPropagatedTracingContextProducerService { inner }
    }
}

pub struct JaegerPropagatedTracingContextProducerService<S> {
    inner: S,
}

impl<S, I> Service<http::Request<I>> for JaegerPropagatedTracingContextProducerService<S>
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
        let hd = req.headers_mut();

        std::thread_local! {
            static PARENT_CTX_MAP: RefCell<HashMap<String, String, FxBuildHasher>> = RefCell::new(HashMap::with_hasher(FxBuildHasher::default()));
        }

        PARENT_CTX_MAP.with(|parent_ctx_map| {
            let mut parent_ctx_map = parent_ctx_map.borrow_mut();
            parent_ctx_map.clear();

            let ctx = tracing::Span::current().context();

            opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&ctx, parent_ctx_map.deref_mut());
            });

            info!("captured context = {:?}", parent_ctx_map);

            for (k, v) in parent_ctx_map.drain() {
                let Ok(k) = k.parse::<HeaderName>() else {
                    continue;
                };
                let Ok(v) = v.parse::<HeaderValue>() else {
                    continue;
                };
                hd.insert(k, v);
            }
        });

        self.inner.call(req)
    }
}
