use async_channel::Receiver;
use channels::deckofcards::{self, DeckID, DrawnCardsInfo};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::ClientBuilder::new().build()?;

    let deck_id = deckofcards::new_deck(client.clone()).await?.deck_id;

    let r = spawn_tasks(client, deck_id, 5);

    while let Ok(msg) = r.recv().await {
        match msg {
            Ok(info) => println!("{}", info.cards.len()),
            Err(err) => println!("failed to retrieve cards: {err}"),
        }
    }

    Ok(())
}

fn spawn_tasks(
    client: reqwest::Client,
    deck_id: DeckID,
    tasks: usize,
) -> Receiver<Result<DrawnCardsInfo, reqwest::Error>> {
    let (s, r) = async_channel::unbounded();

    for _ in 0..tasks {
        let client = client.clone();
        let s = s.clone();
        tokio::spawn(async move {
            let res = deckofcards::draw_cards(client, deck_id, 2)
                .expect("we passed a non-zero number of cards to retrieve")
                .await;
            s.send(res).await.expect("failed to send message");
        });
    }

    r
}
