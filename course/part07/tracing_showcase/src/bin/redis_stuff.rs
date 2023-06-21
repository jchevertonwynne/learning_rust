use std::{sync::{Arc, Mutex}, pin::Pin};

use futures::Future;
use redis::aio::ConnectionLike;
use tower::{Service, ServiceBuilder};
use tracing::info;
use tracing_showcase::{layers::{CheckRequest, CheckResponse, RequestCounterLayer}, tracing_setup::init_tracing};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("redis stuff")?;

    info!("hello!");

    let r = redis::Client::open("redis://127.0.0.1:6379")?;
    let conn = Arc::new(Mutex::new(r.get_tokio_connection().await?));

    let mut service = ServiceBuilder::new()
        .layer(RequestCounterLayer::new(RedisRequestChecker::default()))
        .service(RedisService { conn });

    service.call(redis::Cmd::set("hello", "world")).await?;
    let resp = service.call(redis::Cmd::get("hello")).await?;
    info!(" got resp {resp:?}");

    service.call(redis::Cmd::set("hello", "world2")).await?;
    let resp = service.call(redis::Cmd::get("hello")).await?;
    info!(" got resp {resp:?}");

    info!("goodbye from redis!");

    Ok(())
}

#[derive(Debug, Clone, Default)]
struct RedisRequestChecker {}

#[derive(Debug, Clone, Default)]
struct RedisResponseChecker {}

impl CheckRequest for RedisRequestChecker {
    type Request = redis::Cmd;

    type ResponseChecker = RedisResponseChecker;

    fn is_right_request_type(&self, _req: &Self::Request) -> Option<Self::ResponseChecker> {
        Some(RedisResponseChecker::default())
    }
}

impl CheckResponse for RedisResponseChecker {
    type Response = redis::Value;

    fn is_successful_response(&self, res: &Self::Response) -> bool {
        let expected = "world2".as_bytes().iter().map(|b| *b).collect::<Vec<u8>>();
        res == &redis::Value::Data(expected)
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
        Box::pin(async move { conn.lock().unwrap().req_packed_command(&req).await })
    }
}

#[pin_project::pin_project]
struct RedisFut {
    #[pin]
    fut: Box<dyn Future<Output = redis::Value>>,
}

// impl F
