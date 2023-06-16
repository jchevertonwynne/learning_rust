use grpc::cards_service_client::CardsServiceClient;
use tracing_showcase::grpc;
use tracing_showcase::grpc::{DrawCardsRequest, NewDecksRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = CardsServiceClient::connect("http://127.0.0.1:25565").await?;

    let decks = client
        .new_decks(NewDecksRequest { decks: 3 })
        .await?
        .into_inner();

    let drawn_hands = client
        .draw_cards(DrawCardsRequest {
            deck_id: decks.deck_id.clone(),
            count: 4,
            hands: 20,
        })
        .await?
        .into_inner();

    for hand in drawn_hands.hands {
        println!("{hand:#?}");
    }

    Ok(())
}
