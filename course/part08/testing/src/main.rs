use mongodb::options::ClientOptions;
use testing::{
    config::AppConfig,
    deck_of_cards::DeckOfCardsClient,
    mongo::MongoRecordController,
    state::{
        CardsServiceState,
        DrawCardsRequest,
        DrawCardsResponse,
        NewDecksRequest,
        NewDecksResponse,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let AppConfig {
        mongo,
        deck_of_cards,
    } = AppConfig::load()?;

    let reqwest_client = reqwest::ClientBuilder::new().build()?;

    let mongo_client =
        mongodb::Client::with_options(ClientOptions::parse_connection_string(mongo).await?)?;
    let record_controller = MongoRecordController::new(&mongo_client);

    let state = CardsServiceState::new(
        DeckOfCardsClient::new(deck_of_cards, reqwest_client),
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
