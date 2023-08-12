use async_trait::async_trait;
use hyper::{service::Service, Body};

use tracing::instrument;

use crate::{
    deck_of_cards::DeckOfCardsClient,
    grpc_server_state::{CardsServiceState, DrawCardsRequest, NewDecksRequest},
    mongo::MongoRecordController,
};

pub mod proto {
    tonic::include_proto!("cards");
}

pub struct CardsService<C>
where
    C: Service<http::Request<Body>>,
{
    cards_service_internal: CardsServiceState<C>,
}

impl<C> CardsService<C>
where
    C: Service<http::Request<Body>, Response = http::Response<Body>, Error = hyper::Error>
        + Send
        + Sync
        + 'static,
    C::Future: Send,
{
    pub fn new(
        cards_client: DeckOfCardsClient<C>,
        record_controller: MongoRecordController,
    ) -> Self {
        Self {
            cards_service_internal: CardsServiceState::new(cards_client, record_controller),
        }
    }
}

#[async_trait]
impl<C> proto::cards_service_server::CardsService for CardsService<C>
where
    C: Service<http::Request<Body>, Response = http::Response<Body>, Error = hyper::Error>
        + Send
        + Sync
        + 'static,
    C::Future: Send,
{
    #[instrument(skip(self, request))]
    async fn new_decks(
        &self,
        request: tonic::Request<proto::NewDecksRequest>,
    ) -> Result<tonic::Response<proto::NewDecksResponse>, tonic::Status> {
        let new_decks_request = NewDecksRequest::try_from(request.into_inner())
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

        let new_decks_response = self
            .cards_service_internal
            .new_deck(new_decks_request)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        Ok(tonic::Response::new(new_decks_response.into()))
    }

    #[instrument(skip(self, request))]
    async fn draw_cards(
        &self,
        request: tonic::Request<proto::DrawCardsRequest>,
    ) -> Result<tonic::Response<proto::DrawCardsResponse>, tonic::Status> {
        let draw_cards_request = DrawCardsRequest::try_from(request.into_inner())
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

        let hands = self
            .cards_service_internal
            .draw_cards(draw_cards_request)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        Ok(tonic::Response::new(hands.into()))
    }
}
