use channels::deck_of_cards::{self, DeckID, DrawnCardsInfo};
use tokio::sync::mpsc::Receiver;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::ClientBuilder::new().build()?;

    let deck_id = deck_of_cards::new_deck(client.clone()).await?.deck_id;

    let mut r = spawn_tasks(client, deck_id, 5);

    while let Some(msg) = r.recv().await {
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
    let (s, r) = tokio::sync::mpsc::channel(tasks);

    for _ in 0..tasks {
        let client = client.clone();
        let s = s.clone();
        tokio::spawn(async move {
            let res = deck_of_cards::draw_cards(client, deck_id, 2)
                .expect("we passed a non-zero number of cards to retrieve")
                .await;
            s.send(res).await.expect("failed to send message");
        });
    }

    r
}
