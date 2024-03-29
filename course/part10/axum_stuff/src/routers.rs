use std::{
    sync::{
        atomic::{AtomicBool, AtomicI16, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    body::HttpBody,
    error_handling::HandleErrorLayer,
    extract::{MatchedPath, Path, State},
    handler::Handler,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, IntoMakeService},
    Json,
    Router,
};
use axum_extra::routing::{RouterExt, TypedPath};
use rabbit_stuff::{
    impls::{MyMessage, OtherMessage, Pupil, SchoolAge},
    rabbit::{PublishError, Rabbit, EXCHANGE, MESSAGE_TYPE, MESSAGE_TYPE_2},
};
use serde::{Deserialize, Serialize};
use tower::{
    limit::GlobalConcurrencyLimitLayer,
    load_shed::LoadShedLayer,
    BoxError,
    ServiceBuilder,
};
use tower_http::compression::{CompressionLayer, DefaultPredicate};
use tracing::info;

use crate::tower_stuff::PanicCaptureLayer;

pub fn service(rabbit: Rabbit) -> IntoMakeService<Router> {
    Router::new()
        .route("/endpoint", get(endpoint))
        .with_state(Arc::new(rabbit))
        // curl localhost:25565/hello
        .route(
            "/hello",
            get(hello).post(world), // .layer(EveryOtherRequestLayer::default()),
        )
        // curl localhost:25565/world
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
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    (StatusCode::SERVICE_UNAVAILABLE, err.to_string())
                }))
                .layer(LoadShedLayer::new())
                .layer(GlobalConcurrencyLimitLayer::new(54321)), // across all connections
        )
        .with_state(Arc::new(AtomicI16::default()))
        .into_make_service()
}

fn numbers_subrouter<S, B>() -> Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + Sync + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    Router::new()
        // curl localhost:25565/numbers/1
        .route("/1", get(|| async { "one" }))
        // curl localhost:25565/numbers/2
        .route("/2", get(|| async { (StatusCode::CREATED, "two") }))
        // curl localhost:25565/numbers/5
        .typed_get(|NumbersPath { number }: NumbersPath| async move {
            format!("dynamic number: {number}")
        })
        // curl -v localhost:25565/numbers/divide -X GET --json '{"numerator": 13, "denominator": 5}'
        .route("/divide", get(divide.layer(PanicCaptureLayer)))
        // curl -v localhost:25565/numbers/divide2 -X GET --json '{"numerator": 13, "denominator": 5}'
        .route("/divide2", get(divide2))
    // .layer(EveryOtherRequestLayer::default())
}

#[derive(Debug, TypedPath, Deserialize)]
#[typed_path("/:number")]
struct NumbersPath {
    number: usize,
}

fn a_path_subrouter<S, B>() -> Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + Sync + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    Router::new()
        // curl -v localhost:25565/swap/please
        .route(
            "/:b",
            get(
                |Path((a, b)): Path<(String, String)>, matched_path: MatchedPath| async move {
                    info!(matched_path=?matched_path, "hit the long url endpoint");
                    format!(
                        "if this url was 100 times longer and reversed: /{b}/{a}",
                        b = b.repeat(100),
                        a = a.repeat(100)
                    )
                },
            ),
        )
        .layer(CompressionLayer::<DefaultPredicate>::default())
}

async fn hello(State(counter): State<Arc<AtomicI16>>) -> Response {
    let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
    info!(count = count, "hello endpoint has been hit");
    tokio::time::sleep(Duration::from_secs(1)).await;
    "hello world".into_response()
}

#[derive(Debug, thiserror::Error)]
enum EndpointError {
    #[error("failed to publish rabbit msg: {0}")]
    Publish(#[from] PublishError),
}

impl IntoResponse for EndpointError {
    fn into_response(self) -> Response {
        match self {
            EndpointError::Publish(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
            }
        }
    }
}

async fn endpoint(State(rabbit): State<Arc<Rabbit>>) -> Result<(), EndpointError> {
    tokio::time::sleep(Duration::from_millis(500)).await;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE,
            MyMessage {
                age: 25,
                name: "joseph".into(),
            },
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE,
            MyMessage {
                age: 25,
                name: "\newline encoded".into(),
            },
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE_2,
            OtherMessage {
                school_age: SchoolAge::Primary,
                pupils: vec![
                    Pupil {
                        first_name: "jason".to_string(),
                        second_name: "mccullough".to_string(),
                    },
                    Pupil {
                        first_name: "david".to_string(),
                        second_name: "petran".to_string(),
                    },
                ],
            },
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok(())
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
    let res = numerator
        .checked_div(denominator)
        .map(|result| Json(DivideResult { result }))
        .ok_or(DivideByZeroError)?;

    Ok(res)
}
