use std::{borrow::Borrow, fmt::Debug};

use async_channel::{Receiver, Sender};
use axum::BoxError;
use futures::StreamExt;
use http::StatusCode;
use hyper::{
    body::{Bytes, HttpBody},
    Body,
    Request,
};
use serde::de::DeserializeOwned;
use tokio::task::JoinHandle;
use tower::{Service, ServiceExt};
use tracing::Instrument;
use url::Url;

use crate::model::{DeckID, DeckInfo, DrawnCardsInfo};

pub struct DeckOfCardsClient {
    base_url: Url,
    tx: Sender<PerformRequestMsg>,
}

struct PerformRequestMsg {
    span: tracing::Span,
    req: Request<Body>,
    tx: tokio::sync::oneshot::Sender<JoinHandle<Result<http::Response<Bytes>, ApiError>>>,
}

impl DeckOfCardsClient {
    pub fn new<C, Res>(mut base_url: Url, client: C) -> Self
    where
        C: Service<Request<Body>, Response = http::Response<Res>, Error = hyper::Error>
            + Send
            + 'static,
        C::Future: Send + 'static,
        Res: HttpBody + Send + 'static,
        Res::Data: Send,
        Res::Error: Into<BoxError>,
    {
        let (tx, rx) = async_channel::bounded(32);
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
            .send(PerformRequestMsg { span, req, tx })
            .await
            .expect("actor should always be able to receive messages");

        let body = rx
            .await
            .map_err(ApiError::Recv)?
            .await
            .map_err(ApiError::TaskPanic)??
            .into_body();

        let res = serde_json::from_slice(body.borrow())?;

        Ok(res)
    }
}

async fn service_loop<C, Res>(
    mut client: C,
    mut rx: Receiver<PerformRequestMsg>,
) -> anyhow::Result<()>
where
    C: Service<Request<Body>, Response = http::Response<Res>, Error = hyper::Error>,
    C::Future: Send + 'static,
    Res: HttpBody + Send,
    Res::Data: Send,
    Res::Error: Into<BoxError>,
{
    loop {
        client
            .ready()
            .await
            .map_err(|err| anyhow::anyhow!("failed to check if client is ready: {err}"))?;

        let Some(msg) = rx.next().await else { break };

        let PerformRequestMsg { span, req, tx } = msg;

        let req = client.call(req);
        let handle = tokio::spawn(
            async move {
                let resp = req.await.map_err(ApiError::RequestFailed)?;

                let (parts, body) = resp.into_parts();

                if !parts.status.is_success() {
                    return Err(ApiError::BadStatusCode(parts.status));
                }

                let bytes = hyper::body::to_bytes(body)
                    .await
                    .map_err(|err| ApiError::FailedToReadBody(err.into()))?;

                Ok(http::Response::from_parts(parts, bytes))
            }
            .instrument(span),
        );
        tx.send(handle)
            .unwrap_or_else(|_| panic!("failed to send oneshot response"));
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("failed to build request: {0}")]
    RequestBuildFailure(http::Error),
    #[error("failed to recv response: {0}")]
    Recv(tokio::sync::oneshot::error::RecvError),
    #[error("tokio task panicked: {0}")]
    TaskPanic(tokio::task::JoinError),
    #[error("failed to perform request: {0}")]
    RequestFailed(hyper::Error),
    #[error("got a non-200 status code: {0}")]
    BadStatusCode(StatusCode),
    #[error("failed to read response body: {0}")]
    FailedToReadBody(BoxError),
    #[error("failed to parse response body to json: {0}")]
    Json(#[from] serde_json::Error),
}
