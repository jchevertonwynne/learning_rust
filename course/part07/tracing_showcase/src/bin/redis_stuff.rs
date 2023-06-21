use std::{pin::Pin, sync::Arc};

use futures::Future;
use redis::aio::ConnectionLike;
use tokio::sync::Mutex;
use tower::{Service, ServiceBuilder};
use tracing::{info, info_span, Instrument};
use tracing_showcase::{
    layers::{RequestCounterLayer, SuccessChecker},
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("redis stuff")?;

    let span = info_span!("running redis");
    let entered = span.entered();

    info!("hello!");

    let r = redis::Client::open("redis://127.0.0.1:6379")?;
    let conn = Arc::new(Mutex::new(
        r.get_tokio_connection()
            .instrument(info_span!("connecting to redis"))
            .await?,
    ));

    let mut service = ServiceBuilder::new()
        .layer(RequestCounterLayer::new(RedisChecker::new(
            redis::Value::Data("world2".as_bytes().iter().map(|b| *b).collect::<Vec<u8>>()),
        )))
        .service(RedisService { conn });

    service.call(redis::Cmd::set("hello", "world")).await?;
    let resp = service.call(redis::Cmd::get("hello")).await?;
    info!(" got resp {resp:?}");

    service.call(redis::Cmd::set("hello", "world2")).await?;
    let resp = service.call(redis::Cmd::get("hello")).await?;
    info!(" got resp {resp:?}");

    info!("goodbye from redis!");

    entered.exit();

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

#[derive(Debug, Clone)]
struct RedisChecker {
    wanted: redis::Value,
}

impl RedisChecker {
    fn new(wanted: redis::Value) -> Self {
        Self { wanted }
    }
}

impl SuccessChecker for RedisChecker {
    type Request = redis::Cmd;
    type Response = redis::Value;

    fn should_monitor_response(&self, _req: &Self::Request) -> bool {
        true
    }

    fn is_successful_response(&self, res: &Self::Response) -> bool {
        res == &self.wanted
    }
}

struct RedisService {
    conn: Arc<Mutex<redis::aio::Connection>>,
}

impl Service<redis::Cmd> for RedisService {
    type Response = redis::Value;

    type Error = redis::RedisError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: redis::Cmd) -> Self::Future {
        let conn = self.conn.clone();
        Box::pin(
            async move { conn.lock().await.req_packed_command(&req).await }
                .instrument(info_span!("making a redis request")),
        )
    }
}

#[pin_project::pin_project]
struct RedisFut {
    #[pin]
    fut: Box<dyn Future<Output = redis::Value>>,
}

// impl F
