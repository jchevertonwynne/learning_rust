use futures::{stream::FuturesUnordered, StreamExt};
use hyper::{Body, Client, Request, Uri};
use tower::Service;
use tower_http::decompression::Decompression;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    info!("hello!");

    let http_client = Client::builder().http2_only(true).build_http();
    // let http_client = Client::new();

    let compression_client = Decompression::new(&http_client);

    for i in 1..=2 {
        info!("start of run {i}");
        let mut futs = FuturesUnordered::new();

        for _ in 0..10 {
            let mut compression_client = compression_client.clone();

            let fut = async move {
                let request = Request::builder()
                    .uri("http://localhost:25565/decompression/please".parse::<Uri>()?)
                    // .uri("http://localhost:25565/hello".parse::<Uri>()?)
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
