use axum::http::HeaderValue;
use futures::{stream::FuturesUnordered, StreamExt};
use hyper::{Body, Client, Request, Uri};
use reqwest::header::ACCEPT_ENCODING;
use tower::Service;
use tower_http::decompression::Decompression;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    info!("hello!");

    // let http_client = Client::builder().http2_only(true).build_http();
    let http_client = Client::new();

    let compression_client = tower::service_fn(|mut request: Request<_>| async {
        // request.headers_mut().insert(
        //     http::header::CONNECTION,
        //     HeaderValue::from_str("close").expect("was ascii string"),
        // );
        request.headers_mut().insert(
            ACCEPT_ENCODING,
            HeaderValue::from_str("gzip").expect("was ascii string"),
        );
        Decompression::new(&http_client).call(request).await
    });

    for i in 1..=2 {
        info!("start of run {i}");
        let mut futs = FuturesUnordered::new();

        for _ in 0..10 {
            let mut compression_client = compression_client;

            let fut = async move {
                let request = Request::builder()
                    // .uri("http://localhost:25565/decompression/please".parse::<Uri>()?)
                    .uri("http://localhost:25565/hello".parse::<Uri>()?)
                    .body(Body::empty())?;

                let resp = compression_client.call(request).await?;

                if !resp.status().is_success() {
                    info!(code = ?resp.status(), "hit a bad response");
                }

                debug!("response headers = {headers:?}", headers = resp.headers());

                let body = hyper::body::to_bytes(resp.into_body())
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                info!("body has len {l}", l = body.len());

                Ok::<(), anyhow::Error>(())
            };

            futs.push(fut);
        }

        while let Some(next) = futs.next().await {
            next?;
        }

        info!("end of run {i}");
    }

    info!("goodbye!");

    Ok(())
}
