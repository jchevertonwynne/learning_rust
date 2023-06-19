use pin_project::pin_project;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use tonic::codegen::Service;
use tonic::metadata::{Ascii, MetadataMap, MetadataValue};
use tower::Layer;
use tracing::instrument::Instrumented;
use tracing::{info, info_span, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const TRACE_PROPAGATION_HEADER: &str = "trace-context-propagation";

#[derive(Debug, Clone, Default)]
pub struct GrpcRequestCounterLayer {
    counter_inner: Arc<Mutex<GrpcRequestCounterInner>>,
}

#[derive(Debug, Default)]
pub struct GrpcRequestCounterInner {
    counter: usize,
    counter_success: usize,
}

impl GrpcRequestCounterLayer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> Layer<S> for GrpcRequestCounterLayer {
    type Service = GrpcRequestCounterService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Self::Service {
            counter_inner: self.counter_inner.clone(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcRequestCounterService<S> {
    counter_inner: Arc<Mutex<GrpcRequestCounterInner>>,
    inner: S,
}

impl<S, Req, Res> Service<http::Request<Req>> for GrpcRequestCounterService<S>
where
    S: Service<http::Request<Req>, Response = http::Response<Res>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = GrpcRequestCounterFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<Req>) -> Self::Future {
        if let Some(content_type) = req.headers().get("Content-Type") {
            if content_type == "application/grpc" {
                self.counter_inner.lock().unwrap().counter += 1;
                GrpcRequestCounterFut::Grpc {
                    counter_inner: self.counter_inner.clone(),
                    fut: self.inner.call(req),
                }
            } else {
                GrpcRequestCounterFut::Other(self.inner.call(req))
            }
        } else {
            GrpcRequestCounterFut::Other(self.inner.call(req))
        }
    }
}

#[pin_project(project = GrpcRequestCounterFutProj)]
pub enum GrpcRequestCounterFut<F> {
    Grpc {
        counter_inner: Arc<Mutex<GrpcRequestCounterInner>>,
        #[pin]
        fut: F,
    },
    Other(#[pin] F),
}

impl<F, R, E> Future for GrpcRequestCounterFut<F>
where
    F: Future<Output = Result<http::Response<R>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            GrpcRequestCounterFutProj::Grpc { counter_inner, fut } => {
                let rdy = ready!(fut.poll(cx));
                let mut counters = counter_inner.lock().unwrap();

                if let Ok(resp) = rdy.as_ref() {
                    if resp.status().is_success() {
                        if let Some(grpc_status) = resp.headers().get("Status") {
                            if grpc_status == "0" {
                                counters.counter_success += 1;
                            }
                        } else {
                            counters.counter_success += 1;
                        }
                    }
                }

                let requests_count = counters.counter;
                let requests_success_count = counters.counter_success;
                info!("{requests_success_count}/{requests_count} requests have been successful");
                Poll::Ready(rdy)
            }
            GrpcRequestCounterFutProj::Other(f) => f.poll(cx),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TracingContextPropagatorLayer {}

impl TracingContextPropagatorLayer {
    pub fn new() -> Self {
        TracingContextPropagatorLayer {}
    }
}

impl<S> Layer<S> for TracingContextPropagatorLayer {
    type Service = TracingContextPropagatorService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingContextPropagatorService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct TracingContextPropagatorService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for TracingContextPropagatorService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    ResBody: Default,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = InterceptorFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future {
        // retrieve headers & place empty headers as placeholder
        let metadata = MetadataMap::from_headers(std::mem::take(req.headers_mut()));

        let Some(parent_ctx) = metadata.get(TRACE_PROPAGATION_HEADER) else {
            return InterceptorFut::Fut(self.inner.call(req));
        };

        let parent_ctx_str = match parent_ctx.to_str() {
            Ok(parent_ctx) => parent_ctx,
            Err(err) => {
                return InterceptorFut::Status(tonic::Status::internal(format!(
                    "failed to convert ascii string to str: {err}"
                )))
            }
        };

        let parent_ctx_map = match serde_json::from_str::<HashMap<String, String>>(parent_ctx_str) {
            Ok(parent_ctx) => parent_ctx,
            Err(err) => {
                return InterceptorFut::Status(tonic::Status::internal(format!(
                    "failed to parse parent ctx json: {err}"
                )))
            }
        };

        // put headers back now that we're done with them
        *req.headers_mut() = metadata.into_headers();

        let parent_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&parent_ctx_map)
        });

        let span = info_span!("handling a request", uri = %req.uri());
        span.set_parent(parent_ctx);

        InterceptorFut::FutInstrumented(Instrument::instrument(self.inner.call(req), span))
    }
}

#[pin_project(project = InterceptorFutProj)]
pub enum InterceptorFut<F> {
    Status(tonic::Status),
    FutInstrumented(#[pin] Instrumented<F>),
    Fut(#[pin] F),
}

impl<F, ResBody, E> Future for InterceptorFut<F>
where
    F: Future<Output = Result<http::Response<ResBody>, E>>,
    ResBody: Default,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            InterceptorFutProj::Status(s) => {
                // replace status with cheap to make dummy value
                let s = std::mem::replace(s, tonic::Status::internal(""));
                let (p, _) = s.to_http().into_parts();
                Poll::Ready(Ok(http::Response::from_parts(p, ResBody::default())))
            }
            InterceptorFutProj::FutInstrumented(f) => f.poll(cx),
            InterceptorFutProj::Fut(f) => f.poll(cx),
        }
    }
}

pub fn intercept_outbound(
    mut req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    let ctx = tracing::Span::current().context();

    let ctx_map = opentelemetry::global::get_text_map_propagator(|propagator| {
        let mut propagation_ctx = HashMap::<String, String>::default();
        propagator.inject_context(&ctx, &mut propagation_ctx);
        propagation_ctx
    });

    let ctx_str = match serde_json::to_string(&ctx_map) {
        Ok(ctx_str) => ctx_str,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };

    let ctx_str: MetadataValue<Ascii> = match ctx_str.try_into() {
        Ok(ctx_str) => ctx_str,
        Err(err) => return Err(tonic::Status::internal(err.to_string())),
    };

    req.metadata_mut().insert(TRACE_PROPAGATION_HEADER, ctx_str);
    // req.metadata_mut()
    //     .insert("tracing-parent-context", "yolo".try_into().unwrap());

    Ok(req)
}
