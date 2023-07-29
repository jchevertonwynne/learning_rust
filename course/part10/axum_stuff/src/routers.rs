use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    body::HttpBody,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json,
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::{
    compression::{CompressionLayer, DefaultPredicate},
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{info, Span};

use crate::tower_stuff::EveryOtherRequestLayer;

pub fn main_router<S, B>() -> Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + Sync + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    Router::new()
        .route(
            "/hello",
            get(hello)
                .post(world)
                .layer(EveryOtherRequestLayer::default()),
        )
        .route(
            "/world",
            get(world).layer(axum::middleware::from_fn_with_state(
                Arc::new(AtomicBool::new(false)),
                flip_flop,
            )),
        )
        .nest("/:a", a_path_subrouter())
        .nest("/numbers", numbers_subrouter())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(|_response: &Response, duration: Duration, _span: &Span| {
                    info!("request took {:?} to complete", duration);
                }),
        )
        .with_state(Arc::new(AtomicUsize::default()))
}

fn numbers_subrouter<S, B>() -> Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + Sync + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    Router::new()
        .route("/1", get(|| async { "one" }))
        .route("/2", get(|| async { (StatusCode::CREATED, "two") }))
        .route(
            "/divide",
            get(divide).layer(tower_http::catch_panic::CatchPanicLayer::new()),
        )
        .route("/divide2", get(divide2))
        .route(
            "/:num",
            get(|Path(number): Path<usize>| async move { format!("dynamic number: {number}") }),
        )
        .layer(EveryOtherRequestLayer::default())
}

fn a_path_subrouter<S, B>() -> Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + Sync + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    Router::new()
        .route(
            "/:b",
            get(|Path((a, b)): Path<(String, String)>| async move {
                info!("hit the long url endpoint!");
                format!(
                    "if this url was 100 times longer and reversed: /{b}/{a}",
                    b = b.repeat(100),
                    a = a.repeat(100)
                )
            }),
        )
        .layer(CompressionLayer::<DefaultPredicate>::default())
}

async fn hello(State(counter): State<Arc<AtomicUsize>>) -> Response {
    let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
    info!("hello endpoint has been hit - {count}");
    "hello world".into_response()
}

async fn world() -> impl IntoResponse {
    (StatusCode::ACCEPTED, "HELLO WORLD")
}

async fn flip_flop<B>(
    State(flipper): State<Arc<AtomicBool>>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    if flipper.fetch_xor(true, Ordering::Relaxed) {
        StatusCode::FORBIDDEN.into_response()
    } else {
        next.run(request).await
    }
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
