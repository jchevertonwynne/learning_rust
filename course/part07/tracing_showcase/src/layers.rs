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
use pin_project::pin_project;
use tonic::{
    codegen::Service,
    metadata::{Ascii, MetadataKey, MetadataValue},
};
use tower::Layer;
use tracing::{info, info_span, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub trait CheckRequest: Clone {
    type Request;
    type ResponseChecker;

    fn is_right_request_type(&self, req: &Self::Request) -> Option<Self::ResponseChecker>;
}

pub trait CheckResponse: Clone {
    type Response;

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

impl<I, O> CheckRequest for GrpcCheckRequest<I, O> {
    type Request = http::Request<I>;
    type ResponseChecker = GrpcCheckResponse<O>;

    fn is_right_request_type(&self, req: &http::Request<I>) -> Option<GrpcCheckResponse<O>> {
        info!("headers = {:?}", req.headers());
        matches!(
            req.headers().get("Content-Type").map(|h| h.to_str()),
            Some(Ok("application/grpc"))
        )
        .then_some(GrpcCheckResponse::new())
    }
}

#[derive(Debug)]
pub struct GrpcCheckResponse<O>(PhantomData<O>);

impl<O> Clone for GrpcCheckResponse<O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<O> Default for GrpcCheckResponse<O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<O> GrpcCheckResponse<O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<O> CheckResponse for GrpcCheckResponse<O> {
    type Response = http::Response<O>;

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
pub struct HttpCheckRequest<I, O>(PhantomData<(I, O)>);

impl<I, O> Clone for HttpCheckRequest<I, O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<I, O> Default for HttpCheckRequest<I, O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<I, O> HttpCheckRequest<I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, O> CheckRequest for HttpCheckRequest<I, O> {
    type Request = http::Request<I>;
    type ResponseChecker = HttpCheckResponse<O>;

    fn is_right_request_type(&self, req: &http::Request<I>) -> Option<HttpCheckResponse<O>> {
        info!("headers = {:?}", req.headers());
        Some(HttpCheckResponse::new())
    }
}

#[derive(Debug)]
pub struct HttpCheckResponse<O>(PhantomData<O>);

impl<O> Clone for HttpCheckResponse<O> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<O> Default for HttpCheckResponse<O> {
    fn default() -> Self {
        Self(PhantomData {})
    }
}

impl<O> HttpCheckResponse<O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<O> CheckResponse for HttpCheckResponse<O> {
    type Response = http::Response<O>;

    fn is_successful_response(&self, res: &http::Response<O>) -> bool {
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
            request_checker: self.request_checker.clone(),
            counter_inner: self.counter_inner.clone(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequestCounterService<C, S> {
    request_checker: C,
    counter_inner: Arc<Mutex<RequestCounterInner>>,
    inner: S,
}

impl<C, S, I, O> Service<I> for RequestCounterService<C, S>
where
    C: CheckRequest<Request = I>,
    C::ResponseChecker: CheckResponse<Response = O>,
    S: Service<I, Response = O>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RequestCounterFut<C::ResponseChecker, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        if let Some(response_checker) = self.request_checker.is_right_request_type(&req) {
            RequestCounterFut::Monitored {
                response_checker,
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
        response_checker: C,
        counter_inner: Arc<Mutex<RequestCounterInner>>,
        #[pin]
        fut: F,
    },
    Other(#[pin] F),
}

impl<C, F, O, E> Future for RequestCounterFut<C, F>
where
    C: CheckResponse<Response = O>,
    F: Future<Output = Result<O, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            RequestCounterFutProj::Monitored {
                response_checker,
                counter_inner,
                fut,
            } => {
                let rdy = ready!(fut.poll(cx));
                let mut counters = counter_inner.lock().unwrap();
                counters.counter += 1;

                if let Ok(resp) = rdy.as_ref() {
                    if response_checker.is_successful_response(resp) {
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
pub struct JaegerTracingContextPropagatorLayer {}

impl JaegerTracingContextPropagatorLayer {
    pub fn new() -> Self {
        JaegerTracingContextPropagatorLayer {}
    }
}

impl<S> Layer<S> for JaegerTracingContextPropagatorLayer {
    type Service = JaegerTracingContextPropagatorService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JaegerTracingContextPropagatorService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct JaegerTracingContextPropagatorService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>>
    for JaegerTracingContextPropagatorService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    ResBody: Default,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = StatusOrFuture<Instrumented<S::Future>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
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
                            return StatusOrFuture::Status(tonic::Status::internal(format!(
                                "failed to convert ascii string to str: {err}"
                            )))
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
            StatusOrFuture::Fut(Instrument::instrument(self.inner.call(req), span))
        })
    }
}

#[pin_project(project = StatusOrFutureProj)]
pub enum StatusOrFuture<F> {
    Status(tonic::Status),
    Fut(#[pin] F),
}

impl<F, ResBody, E> Future for StatusOrFuture<F>
where
    F: Future<Output = Result<http::Response<ResBody>, E>>,
    ResBody: Default,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            StatusOrFutureProj::Status(s) => {
                // replace status with cheap to make dummy value
                let s = std::mem::replace(s, tonic::Status::internal(""));
                let (p, _) = s.to_http().into_parts();
                Poll::Ready(Ok(http::Response::from_parts(p, ResBody::default())))
            }
            StatusOrFutureProj::Fut(f) => f.poll(cx),
        }
    }
}

pub fn inject_jaeger_context(
    mut req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
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

        let md = req.metadata_mut();
        for (k, v) in parent_ctx_map.drain() {
            let k = match k.parse::<MetadataKey<Ascii>>() {
                Ok(k) => k,
                Err(err) => return Err(tonic::Status::internal(err.to_string())),
            };
            let v = match v.parse::<MetadataValue<Ascii>>() {
                Ok(v) => v,
                Err(err) => return Err(tonic::Status::internal(err.to_string())),
            };
            md.insert(k, v);
        }

        Ok(req)
    })
}
