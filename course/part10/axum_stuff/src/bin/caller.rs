use axum::http::HeaderValue;
use hyper::{body::HttpBody, Client, Uri};
use reqwest::header::ACCEPT_ENCODING;
use tower::{Service, ServiceExt};
use tower_http::decompression::Decompression;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    info!("hello!");

    let mut client = Decompression::new(Client::new());

    client.ready().await?;

    for _ in 0..2 {
        let request = hyper::Request::builder()
            .header(ACCEPT_ENCODING, HeaderValue::from_str("gzip")?)
            .uri("http://localhost:25565/yolo/swag".parse::<Uri>()?)
            .body(hyper::Body::empty())?;

        let mut resp = client.call(request).await?;

        let mut s = String::new();
        while let Some(d) = resp.body_mut().data().await {
            let bytes = d.map_err(|e| anyhow::anyhow!(e))?;
            s.push_str(std::str::from_utf8(bytes.as_ref())?);
        }

        info!("body has len {l}", l = s.len());
    }

    info!("goodbye!");

    Ok(())
}
