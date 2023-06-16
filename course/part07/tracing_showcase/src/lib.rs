pub mod deck_of_cards;

pub mod grpc {
    tonic::include_proto!("cards");
}
