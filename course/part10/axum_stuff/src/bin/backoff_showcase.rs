use std::future::Ready;

use bytes::Bytes;
use futures::{stream::FuturesUnordered, StreamExt};
use http::{Response, StatusCode, Uri};
use http_body::Full;
use hyper::{Client, Request};
use tower::{retry::Policy, Service, ServiceBuilder};
use tracing::info;

use axum_stuff::tower_stuff::{backoff_strategies::*, BackoffLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    let mut http_service = ServiceBuilder::new()
        .layer(BackoffLayer::new(
            HttpPolicy {
                allowed_retries: 20,
            },
            ExponentialBackoffStrategy,
        ))
        .service(
            Client::builder()
                .http2_only(true)
                .build_http::<Full<Bytes>>(),
        );

    let uri = Uri::try_from("http://localhost:25565/hello")?;
    let mut f = FuturesUnordered::new();
    for _ in 0..2 {
        let req = Request::get(uri.clone()).body(Full::new(Bytes::new()))?;
        f.push(http_service.call(req));
    }

    while let Some(f) = f.next().await {
        let f = f?;
        info!("got resp: {f:?}");
    }

    Ok(())
}

#[derive(Clone)]
struct HttpPolicy {
    allowed_retries: usize,
}

impl<Req, Res, Err> Policy<Request<Req>, Response<Res>, Err> for HttpPolicy
where
    Req: Clone,
{
    type Future = Ready<Self>;

    fn retry(
        &self,
        _req: &Request<Req>,
        result: Result<&Response<Res>, &Err>,
    ) -> Option<Self::Future> {
        if self.allowed_retries == 0 {
            return None;
        }
        match result {
            Ok(res) => {
                if res.status() == StatusCode::SERVICE_UNAVAILABLE {
                    Some(std::future::ready(HttpPolicy {
                        allowed_retries: self.allowed_retries - 1,
                    }))
                } else {
                    None
                }
            }
            Err(_) => Some(std::future::ready(HttpPolicy {
                allowed_retries: self.allowed_retries - 1,
            })),
        }
    }

    fn clone_request(&self, req: &Request<Req>) -> Option<Request<Req>> {
        let mut request = Request::builder()
            .method(req.method().clone())
            .uri(req.uri().clone())
            .version(req.version())
            .body(req.body().clone())
            .ok()?;
        *request.headers_mut() = req.headers().clone();
        Some(request)
    }
}
