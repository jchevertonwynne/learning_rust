// docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

// DECK_OF_CARDS_URL=http://localhost:25566 to use fake deck of cards api

use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::{FutureExt, StreamExt, TryStreamExt};
use grpc::cards_service_server::CardsServiceServer;
use mongodb::bson::doc;
use mongodb::options::UpdateModifications;
use pin_project::pin_project;

use serde::{Deserialize, Serialize};
use tonic::codegen::Service;
use tonic::metadata::MetadataMap;
use tower::Layer;
use tracing::instrument::Instrumented;
use tracing::{info, info_span, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_showcase::deck_of_cards::{DeckID, DeckInfo};
use tracing_showcase::deck_of_cards::{DeckOfCardsClient, DrawnCardsInfo};
use tracing_showcase::{grpc, init_tracing};
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("grpc server")?;

    info!("starting grpc server...");

    let mongo_client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
    let record_controller = MongoRecordController::new(&mongo_client);

    info!("connected to mongo...");

    let client = reqwest::ClientBuilder::default().build()?;
    let url = Url::try_from(
        std::env::var("DECK_OF_CARDS_URL")
            .unwrap_or("https://deckofcardsapi.com".to_string())
            .as_str(),
    )?;
    let cards_client = DeckOfCardsClient::new(url, client);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_success = Arc::new(AtomicUsize::new(0));

    let service = CardsServiceState::new(cards_client, record_controller);

    let addr = ([127, 0, 0, 1], 25565).into();

    info!("serving on {addr}");

    let shutdown = tokio::signal::ctrl_c().map(|_| ());
    tonic::transport::Server::builder()
        .layer(RequestCounterLayer::new(
            counter,
            Arc::clone(&counter_success),
        ))
        .layer(TracingContextPropagatorLayer::new())
        .add_service(CardsServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    info!("goodbye!");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

#[derive(Debug, Clone)]
struct RequestCounterLayer {
    counter: Arc<AtomicUsize>,
    counter_success: Arc<AtomicUsize>,
}

impl RequestCounterLayer {
    fn new(counter: Arc<AtomicUsize>, counter_success: Arc<AtomicUsize>) -> Self {
        Self {
            counter,
            counter_success,
        }
    }
}

impl<S> Layer<S> for RequestCounterLayer {
    type Service = RequestCounterService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        let counter = Arc::clone(&self.counter);
        let counter_success = Arc::clone(&self.counter_success);
        Self::Service {
            counter,
            counter_success,
            inner,
        }
    }
}

#[derive(Debug, Clone)]
struct RequestCounterService<S> {
    counter: Arc<AtomicUsize>,
    counter_success: Arc<AtomicUsize>,
    inner: S,
}

impl<S, T, RespBody> Service<T> for RequestCounterService<S>
where
    S: Service<T, Response = http::Response<RespBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RequestCounterFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        self.counter.fetch_add(1, SeqCst);
        RequestCounterFut {
            counter: self.counter.clone(),
            counter_success: self.counter_success.clone(),
            fut: self.inner.call(req),
        }
    }
}

#[pin_project]
struct RequestCounterFut<F> {
    counter: Arc<AtomicUsize>,
    counter_success: Arc<AtomicUsize>,
    #[pin]
    fut: F,
}

impl<F, T, U> Future for RequestCounterFut<F>
where
    F: Future<Output = Result<http::Response<T>, U>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let resp = this.fut.poll(cx);
        if let Poll::Ready(rdy) = &resp {
            if let Ok(resp) = rdy {
                if let Some(content_type) = resp.headers().get("content-type") {
                    if content_type == "application/grpc" {
                        if let Some(grpc_status) = resp.headers().get("grpc-status") {
                            if grpc_status == "0" {
                                this.counter_success.fetch_add(1, SeqCst);
                            }
                        } else {
                            this.counter_success.fetch_add(1, SeqCst);
                        }
                    } else if resp.status().is_success() {
                        this.counter_success.fetch_add(1, SeqCst);
                    }
                }
            }

            let requests_count = this.counter.load(SeqCst);
            let requests_success_count = this.counter_success.load(SeqCst);
            info!("{requests_success_count}/{requests_count} requests have been successful");
        }
        resp
    }
}

#[derive(Debug, Clone, Default)]
struct TracingContextPropagatorLayer {}

impl TracingContextPropagatorLayer {
    fn new() -> Self {
        TracingContextPropagatorLayer {}
    }
}

impl<S> Layer<S> for TracingContextPropagatorLayer {
    type Service = TracingContextPropagatorService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingContextPropagatorService { inner }
    }
}

#[derive(Debug, Clone)]
struct TracingContextPropagatorService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for TracingContextPropagatorService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    ResBody: Default,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = InterceptorFut<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future {
        // retrieve headers & place empty headers as placeholder
        let metadata = MetadataMap::from_headers(std::mem::take(req.headers_mut()));

        let Some(parent_ctx) = metadata.get("tracing-parent-context") else {
            return InterceptorFut::Fut(self.inner.call(req));
        };

        let parent_ctx_str = match parent_ctx.to_str() {
            Ok(parent_ctx) => parent_ctx,
            Err(err) => {
                return InterceptorFut::Status(tonic::Status::internal(format!(
                    "failed to convert ascii string to str: {err}"
                )))
            }
        };

        let parent_ctx_map = match serde_json::from_str::<HashMap<String, String>>(parent_ctx_str) {
            Ok(parent_ctx) => parent_ctx,
            Err(err) => {
                return InterceptorFut::Status(tonic::Status::internal(format!(
                    "failed to parse parent ctx json: {err}"
                )))
            }
        };

        // put headers back now that we're done with them
        let _ = std::mem::replace(req.headers_mut(), metadata.into_headers());

        let parent_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&parent_ctx_map)
        });

        let span = info_span!("handling a request", uri = %req.uri());
        span.set_parent(parent_ctx);

        InterceptorFut::FutInstrumented(Instrument::instrument(self.inner.call(req), span))
    }
}

#[pin_project(project = InterceptorFutProj)]
enum InterceptorFut<F> {
    Status(tonic::Status),
    FutInstrumented(#[pin] Instrumented<F>),
    Fut(#[pin] F),
}

impl<F, ResBody, E> Future for InterceptorFut<F>
where
    F: Future<Output = Result<http::Response<ResBody>, E>>,
    ResBody: Default,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this {
            InterceptorFutProj::Status(s) => {
                // replace status with cheap to make dummy value
                let s = std::mem::replace(s, tonic::Status::internal(""));
                let (p, _) = s.to_http().into_parts();
                Poll::Ready(Ok(http::Response::from_parts(p, ResBody::default())))
            }
            InterceptorFutProj::FutInstrumented(f) => f.poll(cx),
            InterceptorFutProj::Fut(f) => f.poll(cx),
        }
    }
}

struct CardsServiceState {
    cards_client: DeckOfCardsClient,
    record_controller: MongoRecordController,
}

impl CardsServiceState {
    fn new(cards_client: DeckOfCardsClient, record_controller: MongoRecordController) -> Self {
        Self {
            cards_client,
            record_controller,
        }
    }
}

#[async_trait]
impl grpc::cards_service_server::CardsService for CardsServiceState {
    #[tracing::instrument(skip(self, request))]
    async fn new_decks(
        &self,
        request: tonic::Request<grpc::NewDecksRequest>,
    ) -> Result<tonic::Response<grpc::NewDecksResponse>, tonic::Status> {
        let new_decks_request = match NewDecksRequest::try_from(request.into_inner()) {
            Ok(deck_request) => deck_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let deck_id = match self._new_deck(new_decks_request).await {
            Ok(deck_id) => deck_id,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        Ok(tonic::Response::new(grpc::NewDecksResponse {
            deck_id: deck_id.to_string(),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn draw_cards(
        &self,
        request: tonic::Request<grpc::DrawCardsRequest>,
    ) -> Result<tonic::Response<grpc::DrawCardsResponse>, tonic::Status> {
        let draw_cards_request = match DrawCardsRequest::try_from(request.into_inner()) {
            Ok(cards_request) => cards_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let hands = match self._draw_cards(draw_cards_request).await {
            Ok(hands) => hands,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        let hands = hands
            .into_iter()
            .map(|hand| {
                let cards = hand.cards.iter().map(grpc::Card::from).collect();
                grpc::Hand { cards }
            })
            .collect();

        Ok(tonic::Response::new(grpc::DrawCardsResponse { hands }))
    }
}

#[derive(Debug, thiserror::Error)]
enum NewDeckError {
    #[error("failed to draw deck: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

#[derive(Debug, thiserror::Error)]
enum DrawCardsError {
    #[error("failed to draw cards: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to update mongo: {0}")]
    MongoError(#[from] mongodb::error::Error),
}

impl CardsServiceState {
    #[tracing::instrument(skip(self))]
    async fn _new_deck(&self, new_decks_request: NewDecksRequest) -> Result<DeckID, NewDeckError> {
        let NewDecksRequest { decks } = new_decks_request;

        let DeckInfo { deck_id, .. } = self.cards_client.new_deck(decks).await?;

        info!("created a new deck");

        self.record_controller.create(deck_id).await?;

        info!("stored deck in mongo");

        Ok(deck_id)
    }

    #[tracing::instrument(skip(self))]
    async fn _draw_cards(
        &self,
        draw_cards_request: DrawCardsRequest,
    ) -> Result<Vec<DrawnCardsInfo>, DrawCardsError> {
        let DrawCardsRequest {
            deck_id,
            hands,
            count,
        } = draw_cards_request;

        let hands = self.draw_all_cards(deck_id, hands, count).await?;

        info!("drawn all cards");

        self.record_controller.increment_count(deck_id).await?;

        info!("incremented count in mongo");

        Ok(hands)
    }

    #[tracing::instrument(skip(self))]
    async fn draw_all_cards(
        &self,
        deck_id: DeckID,
        hands: usize,
        count: u8,
    ) -> Result<Vec<DrawnCardsInfo>, reqwest::Error> {
        // (0..hands)
        //     .map(|_| self.cards_client.draw_cards(deck_id, count))
        //     .collect::<FuturesUnordered<_>>()
        futures::stream::iter((0..hands).map(|_| self.cards_client.draw_cards(deck_id, count)))
            .buffer_unordered(5)
            .try_collect()
            .await
    }
}

#[derive(Debug)]
struct NewDecksRequest {
    decks: usize,
}

#[derive(Debug, thiserror::Error)]
enum NewDecksRequestValidationError {
    #[error("count must be a positive integer")]
    InvalidDeckCount,
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<grpc::NewDecksRequest> for NewDecksRequest {
    type Error = NewDecksRequestValidationError;

    fn try_from(value: grpc::NewDecksRequest) -> Result<Self, Self::Error> {
        let grpc::NewDecksRequest { decks } = value;

        let Ok(decks) = usize::try_from(decks) else {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        };

        if decks == 0 {
            return Err(NewDecksRequestValidationError::InvalidDeckCount);
        }

        Ok(NewDecksRequest { decks })
    }
}

#[derive(Debug)]
struct DrawCardsRequest {
    deck_id: DeckID,
    hands: usize,
    count: u8,
}

#[derive(Debug, thiserror::Error)]
enum DrawCardsRequestValidationError {
    #[error("a deck id must be 12 lowercase letters and numbers")]
    DeckID,
    #[error("hands must be a positive integer")]
    Hands,
    #[error("count must be a positive u8 value")]
    Count,
    #[error("failed to parse trace context: {0}")]
    TraceContextParse(#[from] serde_json::Error),
}

impl TryFrom<grpc::DrawCardsRequest> for DrawCardsRequest {
    type Error = DrawCardsRequestValidationError;

    fn try_from(value: grpc::DrawCardsRequest) -> Result<Self, Self::Error> {
        let grpc::DrawCardsRequest {
            deck_id,
            hands,
            count,
        } = value;

        let Ok(deck_id) = DeckID::try_from(deck_id.as_str()) else {
            return Err(DrawCardsRequestValidationError::DeckID);
        };

        let Ok(count) =  u8::try_from(count) else {
            return Err(DrawCardsRequestValidationError::Count);
        };

        let Ok(hands) = usize::try_from(hands) else {
            return Err(DrawCardsRequestValidationError::Hands);
        };

        Ok(DrawCardsRequest {
            deck_id,
            hands,
            count,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct InteractionRecord {
    deck_id: String,
    count: usize,
}

struct MongoRecordController {
    interactions: mongodb::Collection<InteractionRecord>,
}

impl MongoRecordController {
    fn new(client: &mongodb::Client) -> Self {
        let collection = client
            .database("tracing_showcase")
            .collection("interactions");
        Self {
            interactions: collection,
        }
    }

    #[tracing::instrument(skip(self))]
    async fn create(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("creating a new record");
        self.interactions
            .insert_one(
                InteractionRecord {
                    deck_id: deck_id.to_string(),
                    count: 0,
                },
                None,
            )
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn increment_count(&self, deck_id: DeckID) -> mongodb::error::Result<()> {
        info!("incrementing count");
        self.interactions
            .update_one(
                doc! { "deck_id": deck_id.to_string() },
                UpdateModifications::Document(doc! { "$inc": { "count": 1 } }),
                None,
            )
            .await?;
        Ok(())
    }
}
