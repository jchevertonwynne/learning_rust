use async_trait::async_trait;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{ready, Context, Poll},
};

use fxhash::FxBuildHasher;
use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    HeaderMap,
    HeaderName,
    HeaderValue,
};
use pin_project::pin_project;
use tonic::{codegen::Service, metadata::MetadataMap};
use tower::Layer;
use tracing::{error, info, info_span, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub trait SuccessChecker: Clone {
    type Request;
    type Response;

    fn should_monitor_response(&self, req: &Self::Request) -> bool;
    fn is_successful_response(&self, res: &Self::Response) -> bool;
}

#[derive(Debug)]
pub struct GrpcCheckRequest<I, O>(PhantomData<(I, O)>);

impl<I, O> Clone for GrpcCheckRequest<I, O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<I, O> Default for GrpcCheckRequest<I, O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<I, O> GrpcCheckRequest<I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, O> SuccessChecker for GrpcCheckRequest<I, O> {
    type Request = http::Request<I>;
    type Response = http::Response<O>;

    fn should_monitor_response(&self, req: &http::Request<I>) -> bool {
        info!("headers = {:?}", req.headers());
        matches!(
            req.headers().get("Content-Type").map(|h| h.to_str()),
            Some(Ok("application/grpc"))
        )
    }

    fn is_successful_response(&self, res: &http::Response<O>) -> bool {
        info!("headers = {:?}", res.headers());
        res.status().is_success()
            && res
                .headers()
                .get("grpc-status")
                .map(|grpc_status| grpc_status == "0")
                .unwrap_or(true)
    }
}

#[derive(Debug)]
pub struct HttpChecker<I, O>(PhantomData<(I, O)>);

impl<I, O> Clone for HttpChecker<I, O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<I, O> Default for HttpChecker<I, O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<I, O> HttpChecker<I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, O> SuccessChecker for HttpChecker<I, O> {
    type Request = http::Request<I>;
    type Response = http::Response<O>;

    fn should_monitor_response(&self, req: &http::Request<I>) -> bool {
        info!("headers = {:?}", req.headers());
        true
    }

    fn is_successful_response(&self, res: &Self::Response) -> bool {
        info!("headers = {:?}", res.headers());
        res.status().is_success()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RequestCounterLayer<C> {
    request_checker: C,
    counter_inner: Arc<Mutex<RequestCounterInner>>,
}

#[derive(Debug, Default)]
pub struct RequestCounterInner {
    counter: usize,
    counter_success: usize,
}

impl<C> RequestCounterLayer<C> {
    pub fn new(request_checker: C) -> Self {
        Self {
            request_checker,
            counter_inner: Default::default(),
        }
    }
}

impl<C, S> Layer<S> for RequestCounterLayer<C>
where
    C: Clone,
{
    type Service = RequestCounterService<C, S>;

    fn layer(&self, inner: S) -> Self::Service {
        Self::Service {
            req_res_checker: self.request_checker.clone(),
            counter_inner: self.counter_inner.clone(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequestCounterService<C, S> {
    req_res_checker: C,
    counter_inner: Arc<Mutex<RequestCounterInner>>,
    inner: S,
}

impl<C, S, I, O> Service<I> for RequestCounterService<C, S>
where
    C: SuccessChecker<Request = I, Response = O>,
    S: Service<I, Response = O>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RequestCounterFut<C, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        if self.req_res_checker.should_monitor_response(&req) {
            RequestCounterFut::Monitored {
                req_res_checker: self.req_res_checker.clone(),
                counter_inner: self.counter_inner.clone(),
                fut: self.inner.call(req),
            }
        } else {
            RequestCounterFut::Other(self.inner.call(req))
        }
    }
}

#[pin_project(project = RequestCounterFutProj)]
pub enum RequestCounterFut<C, F> {
    Monitored {
        req_res_checker: C,
        counter_inner: Arc<Mutex<RequestCounterInner>>,
        #[pin]
        fut: F,
    },
    Other(#[pin] F),
}

impl<C, F, O, E> Future for RequestCounterFut<C, F>
where
    C: SuccessChecker<Response = O>,
    F: Future<Output = Result<O, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            RequestCounterFutProj::Monitored {
                req_res_checker,
                counter_inner,
                fut,
            } => {
                let rdy = ready!(fut.poll(cx));
                let mut counters = counter_inner.lock().unwrap();
                counters.counter += 1;

                if let Ok(resp) = rdy.as_ref() {
                    if req_res_checker.is_successful_response(resp) {
                        counters.counter_success += 1;
                    }
                }

                let requests_count = counters.counter;
                let requests_success_count = counters.counter_success;
                info!("{requests_success_count}/{requests_count} requests have been successful");
                Poll::Ready(rdy)
            }
            RequestCounterFutProj::Other(f) => f.poll(cx),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct JaegerPropagatedTracingContextConsumerLayer {}

impl JaegerPropagatedTracingContextConsumerLayer {
    pub fn new() -> Self {
        JaegerPropagatedTracingContextConsumerLayer::default()
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
    O: Default,
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
                            error!("failed to convert ascii string to str: {err}");
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

#[derive(Debug, thiserror::Error)]
enum InvalidHeaderError {
    #[error(transparent)]
    Name(#[from] InvalidHeaderName),
    #[error(transparent)]
    Value(#[from] InvalidHeaderValue),
}

fn inject_tracing_context(hd: &mut HeaderMap) -> Result<(), InvalidHeaderError> {
    std::thread_local! {
        static PARENT_CTX_MAP: RefCell<HashMap<String, String, FxBuildHasher>> = RefCell::new(HashMap::with_hasher(FxBuildHasher::default()));
    }

    PARENT_CTX_MAP.with::<_, Result<(), InvalidHeaderError>>(|parent_ctx_map| {
        let mut parent_ctx_map = parent_ctx_map.borrow_mut();
        parent_ctx_map.clear();

        let ctx = tracing::Span::current().context();

        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&ctx, parent_ctx_map.deref_mut());
        });

        for (k, v) in parent_ctx_map.drain() {
            let k = k.parse::<HeaderName>()?;
            let v = v.parse::<HeaderValue>()?;
            hd.insert(k, v);
        }

        Ok(())
    })
}

pub fn jaeger_tracing_context_propagator(
    mut req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    let mut hd = std::mem::take(req.metadata_mut()).into_headers();
    inject_tracing_context(&mut hd).map_err(|err| tonic::Status::internal(err.to_string()))?;
    *req.metadata_mut() = MetadataMap::from_headers(hd);

    Ok(req)
}

#[derive(Debug, Default)]
pub struct JaegerContextPropagatorMiddleware {}

impl JaegerContextPropagatorMiddleware {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl reqwest_middleware::Middleware for JaegerContextPropagatorMiddleware {
    async fn handle(
        &self,
        mut req: reqwest::Request,
        extensions: &'_ mut task_local_extensions::Extensions,
        next: reqwest_middleware::Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        inject_tracing_context(req.headers_mut())
            .map_err(|err| anyhow::anyhow!("failed to parse headers: {err}"))?;
        next.run(req, extensions).await
    }
}
