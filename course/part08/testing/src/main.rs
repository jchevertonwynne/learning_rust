use testing::{
    config::AppConfig,
    deck_of_cards::DeckOfCardsClient,
    mongo::MongoRecordController,
    state::{DeckService, DrawCardsRequest, DrawCardsResponse, NewDecksRequest, NewDecksResponse},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load()?;

    let reqwest_client = reqwest::ClientBuilder::new().build()?;

    let mongo_client =
        mongodb::Client::with_uri_str(config.mongo_config.connection_string.as_str()).await?;
    let record_controller =
        MongoRecordController::new(&mongo_client, config.mongo_config.database_info);

    let state = DeckService::new(
        DeckOfCardsClient::new(config.deck_of_cards, reqwest_client),
        record_controller,
    );

    let NewDecksResponse { deck_id } = state.new_deck(NewDecksRequest { decks: 20 }).await?;

    let DrawCardsResponse { hands } = state
        .draw_cards(DrawCardsRequest {
            deck_id,
            hands: 20,
            count: 5,
        })
        .await?;

    for hand in hands {
        println!("got hand! hand = {hand:#?}");
    }

    Ok(())
}
