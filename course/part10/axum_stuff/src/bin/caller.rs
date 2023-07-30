use axum::http::HeaderValue;
use hyper::{Body, Client, Request, Uri};
use reqwest::header::ACCEPT_ENCODING;
use tower::Service;
use tower_http::decompression::Decompression;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    axum_stuff::tracing_config::init()?;

    info!("hello!");

    let http_client = Client::new();

    let mut compression_client = tower::service_fn(|mut request: Request<_>| async {
        request.headers_mut().insert(
            ACCEPT_ENCODING,
            HeaderValue::from_str("gzip").expect("was ascii string"),
        );
        Decompression::new(&http_client).call(request).await
    });

    for _ in 0..2 {
        let request = Request::builder()
            .uri("http://localhost:25565/yolo/swag".parse::<Uri>()?)
            .body(Body::empty())?;

        let resp = compression_client.call(request).await?;

        let (_, body) = resp.into_parts();

        let body = hyper::body::to_bytes(body)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        info!("body has len {l}", l = body.len());
    }

    info!("goodbye!");

    Ok(())
}
