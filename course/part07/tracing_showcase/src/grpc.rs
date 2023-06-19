use async_trait::async_trait;
use tracing::instrument;

use crate::service::{CardsServiceInternal, NewDecksRequest, DrawCardsRequest};

pub mod proto {
    tonic::include_proto!("cards");
}

pub struct CardsServiceState {
    cards_service_internal: CardsServiceInternal,
}

impl CardsServiceState {
    pub fn new(cards_service_internal: CardsServiceInternal) -> Self {
        Self {
            cards_service_internal,
        }
    }
}

#[async_trait]
impl proto::cards_service_server::CardsService for CardsServiceState {
    #[instrument(skip(self, request))]
    async fn new_decks(
        &self,
        request: tonic::Request<proto::NewDecksRequest>,
    ) -> Result<tonic::Response<proto::NewDecksResponse>, tonic::Status> {
        let new_decks_request = match NewDecksRequest::try_from(request.into_inner()) {
            Ok(deck_request) => deck_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let deck_id = match self
            .cards_service_internal
            .new_deck(new_decks_request)
            .await
        {
            Ok(deck_id) => deck_id,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        Ok(tonic::Response::new(proto::NewDecksResponse {
            deck_id: deck_id.to_string(),
        }))
    }

    #[instrument(skip(self, request))]
    async fn draw_cards(
        &self,
        request: tonic::Request<proto::DrawCardsRequest>,
    ) -> Result<tonic::Response<proto::DrawCardsResponse>, tonic::Status> {
        let draw_cards_request = match DrawCardsRequest::try_from(request.into_inner()) {
            Ok(cards_request) => cards_request,
            Err(err) => return Err(tonic::Status::invalid_argument(err.to_string())),
        };

        let hands = match self
            .cards_service_internal
            .draw_cards(draw_cards_request)
            .await
        {
            Ok(hands) => hands,
            Err(err) => return Err(tonic::Status::internal(err.to_string())),
        };

        let hands = hands
            .into_iter()
            .map(|hand| {
                let cards = hand.cards.iter().map(proto::Card::from).collect();
                proto::Hand { cards }
            })
            .collect();

        Ok(tonic::Response::new(proto::DrawCardsResponse { hands }))
    }
}
