use std::{pin::Pin, sync::Arc};

use futures::Future;
use redis::{aio::ConnectionLike, ToRedisArgs};
use tokio::sync::Mutex;
use tower::{Service, ServiceBuilder};
use tracing::{info, info_span, Instrument};
use tracing_showcase::{
    layers::request_counter::{RequestCounterLayer, SuccessChecker},
    tracing_setup::init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("redis stuff")?;

    info!("hello!");

    let span = info_span!("running redis");

    async {
        let r = redis::Client::open("redis://127.0.0.1:6379")?;
        let conn = Arc::new(Mutex::new(
            r.get_tokio_connection()
                .instrument(info_span!("getting async redis connection"))
                .await?,
        ));

        let mut service = ServiceBuilder::new()
            .layer(RequestCounterLayer::new(RedisGetChecker::new(
                redis::Value::Data("world2".as_bytes().to_vec()),
            )))
            .service(RedisService { conn });

        service.call(RedisRequest::set("hello", "world")).await?;
        let resp = service.call(RedisRequest::get("hello")).await?;
        info!(" got resp {resp:?}");

        service.call(RedisRequest::set("hello", "world2")).await?;
        let resp = service.call(RedisRequest::get("hello")).await?;
        info!(" got resp {resp:?}");

        Ok::<(), anyhow::Error>(())
    }
    .instrument(span)
    .await?;

    info!("goodbye from redis!");

    Ok(())
}

struct RedisRequest {
    cmd: redis::Cmd,
    request_type: RedisRequestType,
}

impl RedisRequest {
    fn get<K: ToRedisArgs>(key: K) -> Self {
        RedisRequest {
            cmd: redis::Cmd::get(key),
            request_type: RedisRequestType::Get,
        }
    }

    fn set<K: ToRedisArgs, V: ToRedisArgs>(key: K, val: V) -> Self {
        RedisRequest {
            cmd: redis::Cmd::set(key, val),
            request_type: RedisRequestType::Set,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RedisRequestType {
    Set,
    Get,
}

#[derive(Debug, Clone)]
struct RedisGetChecker {
    wanted: redis::Value,
}

impl RedisGetChecker {
    fn new(wanted: redis::Value) -> Self {
        Self { wanted }
    }
}

impl SuccessChecker for RedisGetChecker {
    type Request = RedisRequest;
    type Response = redis::Value;

    fn should_monitor_response(&self, _req: &Self::Request) -> bool {
        matches!(_req.request_type, RedisRequestType::Get)
    }

    fn is_successful_response(&self, res: &Self::Response) -> bool {
        res == &self.wanted
    }
}

struct RedisService {
    conn: Arc<Mutex<redis::aio::Connection>>,
}

impl Service<RedisRequest> for RedisService {
    type Response = redis::Value;

    type Error = redis::RedisError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: RedisRequest) -> Self::Future {
        let RedisRequest { cmd, request_type } = req;
        let conn = self.conn.clone();
        Box::pin(
            async move { conn.lock().await.req_packed_command(&cmd).await }
                .instrument(info_span!("making a redis request", request_type = ?request_type)),
        )
    }
}
