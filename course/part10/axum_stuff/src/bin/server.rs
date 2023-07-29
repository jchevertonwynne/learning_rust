use std::{
    convert::Infallible,
    future::Future,
    net::{SocketAddr, TcpListener},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE},
        HeaderMap,
        HeaderValue,
        Request,
        StatusCode,
    },
    middleware::Next,
    response::{AppendHeaders, IntoResponse, Response},
    routing::get,
    Json,
    Router,
    Server,
};
use futures::FutureExt;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};
use tower_http::compression::{CompressionLayer, DefaultPredicate};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    info!("hello!");

    let numbers_sub_router = Router::new()
        .route("/1", get(|| async { "one" }))
        .route("/2", get(|| async { (StatusCode::CREATED, "two") }))
        .route("/divide", get(divide).layer(tower_http::catch_panic::CatchPanicLayer::new()))
        .route("/divide2", get(divide2))
        .route(
            "/:num",
            get(|Path(number): Path<usize>| async move { format!("dynamic number: {number}") }),
        )
        .layer(EveryOtherRequestLayer::default());

    let a_sub_router = Router::new()
        .route(
            "/:b",
            get(|Path((a, b)): Path<(String, String)>| async move {
                format!("reversed: /{b}/{a}", b = b.repeat(100), a = a.repeat(100))
            }),
        )
        .layer(CompressionLayer::<DefaultPredicate>::default());

    // let flip_flop_layer =
    //     axum::middleware::from_fn_with_state(Arc::new(AtomicBool::new(false)), flip_flop);

    let router = Router::new()
        .route(
            "/hello",
            get(hello)
                .post(world)
                .layer(EveryOtherRequestLayer::default()),
        )
        .route("/world", get(world))
        .nest("/:a", a_sub_router)
        .nest("/numbers", numbers_sub_router)
        .with_state(Arc::new(AtomicUsize::default()));
    // .layer(EveryOtherRequestLayer::default())
    // .layer(flip_flop_layer);

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 25565)))?;

    let shutdown = tokio::signal::ctrl_c().map(|_| ());

    let server = Server::from_tcp(listener)?
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown);

    server.await?;

    info!("goodbye!");

    Ok(())
}

async fn flip_flop<B>(
    State(state): State<Arc<AtomicBool>>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    if state.fetch_xor(true, Ordering::Relaxed) {
        StatusCode::FORBIDDEN.into_response()
    } else {
        next.run(request).await
    }
}

async fn hello(State(counter): State<Arc<AtomicUsize>>) -> Response {
    let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
    info!("hello endpoint has been hit - {count}");
    "hello world".into_response()
}

async fn world() -> impl IntoResponse {
    (StatusCode::ACCEPTED, "HELLO WORLD")
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
struct Numbers {
    numerator: isize,
    denominator: isize,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
struct DivideResult {
    result: isize,
}

async fn divide(
    Json(Numbers {
        numerator,
        denominator,
    }): Json<Numbers>,
) -> Json<DivideResult> {
    Json(DivideResult {
        result: numerator / denominator,
    })
}

#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("denominator cannot be zero")]
struct DivideByZeroError;

impl IntoResponse for DivideByZeroError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, "numerator cannot be zero").into_response()
    }
}

async fn divide2(
    Json(Numbers {
        numerator,
        denominator,
    }): Json<Numbers>,
) -> Result<Json<DivideResult>, DivideByZeroError> {
    numerator
        .checked_div(denominator)
        .map(|result| Json(DivideResult { result }))
        .ok_or(DivideByZeroError)
}

#[derive(Debug, Clone, Default)]
struct EveryOtherRequestLayer {
    switch: Arc<AtomicBool>,
}

impl<S> Layer<S> for EveryOtherRequestLayer {
    type Service = EveryOtherService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        EveryOtherService::new(inner, self.switch.clone())
    }
}

#[derive(Debug, Clone)]
struct EveryOtherService<S> {
    inner: S,
    switch: Arc<AtomicBool>,
}

impl<S> EveryOtherService<S> {
    fn new(inner: S, switch: Arc<AtomicBool>) -> EveryOtherService<S> {
        EveryOtherService { inner, switch }
    }
}

impl<S, I> Service<S> for EveryOtherService<I>
where
    I: Service<S, Response = Response>,
{
    type Response = I::Response;
    type Error = I::Error;
    type Future = EveryOtherFut<I::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: S) -> Self::Future {
        if self.switch.fetch_xor(true, Ordering::Relaxed) {
            EveryOtherFut::Failed
        } else {
            EveryOtherFut::Fut(self.inner.call(req))
        }
    }
}

#[pin_project(project = EveryOtherFutProjection)]
enum EveryOtherFut<F> {
    Failed,
    Fut(#[pin] F),
}

impl<F, E> Future for EveryOtherFut<F>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            EveryOtherFutProjection::Failed => {
                Poll::Ready(Ok(StatusCode::FORBIDDEN.into_response()))
            }
            EveryOtherFutProjection::Fut(f) => f.poll(cx),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct PanicCaptureLayer;

impl<S> Layer<S> for PanicCaptureLayer {
    type Service = PanicCaptureService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PanicCaptureService { inner }
    }
}

#[derive(Debug, Clone)]
struct PanicCaptureService<S> {
    inner: S,
}

impl<S, I> Service<I> for PanicCaptureService<S>
where
    S: Service<I, Response = Response, Error = Infallible>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = PanicCaptureFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: I) -> Self::Future {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.inner.call(req))) {
            Ok(fut) => PanicCaptureFut::Fut(fut),
            Err(_) => PanicCaptureFut::Panicked,
        }
    }
}

#[pin_project(project = PanicCaptureFutProjection)]
enum PanicCaptureFut<F> {
    Panicked,
    Fut(#[pin] F),
}

impl<F> Future for PanicCaptureFut<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            PanicCaptureFutProjection::Panicked => Poll::Ready(Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                "PanicCaptureService::call panicked",
            )
                .into_response())),
            PanicCaptureFutProjection::Fut(fut) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fut.poll(cx))) {
                    Ok(polled) => polled,
                    Err(_) => Poll::Ready(Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PanicCaptureFut::poll panicked",
                    )
                        .into_response())),
                }
            }
        }
    }
}
