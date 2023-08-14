use std::{borrow::Borrow, fmt::Debug, future::Future};

use async_channel::{Receiver, Sender};
use futures::StreamExt;
use http::StatusCode;
use hyper::{body::HttpBody, Body, Request};
use serde::de::DeserializeOwned;
use tower::{Service, ServiceExt};
use url::Url;

use crate::model::{DeckID, DeckInfo, DrawnCardsInfo};

pub struct DeckOfCardsClient<F> {
    base_url: Url,
    tx: Sender<Msg<F>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("failed to build request: {0}")]
    RequestBuildFailure(http::Error),
    #[error("failed to perform request: {0}")]
    RequestPerformError(hyper::Error),
    #[error("got a non-200 status code: {0}")]
    FailedRequestError(StatusCode),
    #[error("failed to read response body: {0}")]
    FailedToReadBody(anyhow::Error),
    #[error("failed to parse response body to json: {0}")]
    JsonError(#[from] serde_json::Error),
}

async fn service_loop<C>(mut client: C, mut rx: Receiver<Msg<C::Future>>) -> anyhow::Result<()>
where
    C: Service<Request<Body>, Error = hyper::Error>,
{
    while let Some(msg) = rx.next().await {
        let Msg { span, req, tx } = msg;
        let _entered = span.enter();
        client.ready().await?;
        let f = client.call(req);
        tx.send(f)
            .unwrap_or_else(|_| panic!("failed to send oneshot response"));
    }

    Ok(())
}

struct Msg<F> {
    span: tracing::Span,
    req: Request<Body>,
    tx: tokio::sync::oneshot::Sender<F>,
}

impl<F, B> DeckOfCardsClient<F>
where
    F: Future<Output = Result<http::Response<B>, hyper::Error>>,
    B: HttpBody,
    B::Error: std::error::Error,
{
    pub fn new<C>(mut base_url: Url, client: C) -> Self
    where
        C: Service<Request<Body>, Error = hyper::Error, Future = F> + Send + Sync + 'static,
        C::Future: Send + Sync,
        B: Send + Sync + 'static,
    {
        let (tx, rx): (Sender<Msg<C::Future>>, _) = async_channel::bounded(32);
        tokio::spawn(service_loop(client, rx));
        base_url.set_path("");
        base_url.set_query(None);
        Self { base_url, tx }
    }

    #[tracing::instrument(skip(self))]
    pub async fn new_deck(&self, decks: usize) -> Result<DeckInfo, ApiError> {
        let mut url = self.base_url.clone();
        url.set_path("/api/deck/new/shuffle/");
        url.set_query(Some(&format!("deck_count={decks}")));

        let req = Request::get(url.as_str())
            .body(Body::empty())
            .map_err(ApiError::RequestBuildFailure)?;

        self.send_and_parse_json(req).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn draw_cards(&self, deck_id: DeckID, n: u8) -> Result<DrawnCardsInfo, ApiError> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("/api/deck/{deck_id}/draw/"));
        url.set_query(Some(&format!("count={n}")));

        let req = Request::get(url.as_str())
            .body(Body::empty())
            .map_err(ApiError::RequestBuildFailure)?;

        self.send_and_parse_json(req).await
    }

    async fn send_and_parse_json<T: DeserializeOwned>(
        &self,
        req: Request<Body>,
    ) -> Result<T, ApiError> {
        let span = tracing::Span::current();
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.tx
            .send(Msg { span, req, tx })
            .await
            .expect("actor should always be ready");

        let resp = rx
            .await
            .expect("should always get a response")
            .await
            .map_err(ApiError::RequestPerformError)?;

        let (parts, body) = resp.into_parts();

        if !parts.status.is_success() {
            return Err(ApiError::FailedRequestError(parts.status));
        }

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(|err| ApiError::FailedToReadBody(anyhow::anyhow!("{err}")))?;

        let res = serde_json::from_slice(bytes.borrow())?;

        Ok(res)
    }
}
