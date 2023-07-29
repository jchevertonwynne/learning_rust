use axum::http::HeaderValue;
use reqwest::{header::ACCEPT_ENCODING, Method};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::ClientBuilder::default().build()?;
    let mut req =
        reqwest::Request::new(Method::GET, "http://localhost:25565/yolo/swag".parse()?);
    req.headers_mut()
        .insert(ACCEPT_ENCODING, HeaderValue::from_str("gzip")?);
    let resp = client.execute(req).await?;
    println!("headers = {:?}", resp.headers());
    let body = resp.bytes().await?;

    println!("body has len {l}", l = body.len());
    Ok(())
}
