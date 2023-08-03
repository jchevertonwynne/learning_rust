use futures::{stream::FuturesUnordered, StreamExt};
use http::{Response, StatusCode, Uri};
use hyper::{Client, Request};
use std::future::Ready;
use tower::{retry::Policy, Service, ServiceBuilder};
use tracing::info;

use axum_stuff::tower_stuff::{backoff_strategies::*, BackoffLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    let http_client = Client::builder().build_http::<String>();
    let mut s2 = ServiceBuilder::new()
        .layer(BackoffLayer::new(HttpPolicy, ExponentialBackoffStrategy))
        .service(http_client);

    let mut f = FuturesUnordered::new();
    for _ in 0..2 {
        let req =
            Request::get(Uri::try_from("http://localhost:25565/hello")?).body(String::new())?;
        f.push(s2.call(req));
    }

    while let Some(f) = f.next().await {
        let f = f?;
        info!("got resp: {f:?}");
    }

    Ok(())
}

#[derive(Clone)]
struct HttpPolicy;

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
        match result {
            Ok(res) => {
                if res.status() == StatusCode::SERVICE_UNAVAILABLE {
                    Some(std::future::ready(HttpPolicy))
                } else {
                    None
                }
            }
            Err(_) => Some(std::future::ready(HttpPolicy)),
        }
    }

    fn clone_request(&self, req: &Request<Req>) -> Option<Request<Req>> {
        info!("cloning request!");
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
