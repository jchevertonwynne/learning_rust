use reqwest::{Client};
use channels::deck_of_cards::{self, CantBeZeroError, DeckID, DrawnCardsInfo};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::ClientBuilder::new().build()?;

    let deck_id = deck_of_cards::new_deck(client.clone()).await?.deck_id;

    // communicating over channels
    let mut r = spawn_tasks(client.clone(), deck_id, 5);

    while let Some(msg) = r.recv().await {
        match msg {
            Ok(info) => println!("{}", info.cards.len()),
            Err(err) => println!("failed to retrieve cards: {err}"),
        }
    }


    // actor loop + oneshot channels
    let (s, r) = async_channel::unbounded();

    let handle = tokio::spawn(actor_loop(client, r));

    let (os, or) = tokio::sync::oneshot::channel();
    s.send(ActionRequest{
        deck_id,
        cards: 3,
        response: os,
    }).await?;

    let response = or.await??;

    println!("response from actor loop: {response:?}");

    handle.await?;

    Ok(())
}

fn spawn_tasks(
    client: Client,
    deck_id: DeckID,
    tasks: usize,
) -> mpsc::Receiver<Result<DrawnCardsInfo, reqwest::Error>> {
    let (s, r) = mpsc::channel(tasks);

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

struct ActionRequest {
    deck_id: DeckID,
    cards: u8,
    response: tokio::sync::oneshot::Sender<Result<DrawnCardsInfo, ActionError>>
}

#[derive(thiserror::Error, Debug)]
enum ActionError {
    #[error("cards to draw cant be zero")]
    CantBeZeroError(#[from] CantBeZeroError),
    #[error("failed to send request: {0}")]
    ReqwestError(#[from] reqwest::Error)
}

async fn actor_loop(client: Client, r: async_channel::Receiver<ActionRequest>) {
    while let Ok(ActionRequest{deck_id, cards, response}) = r.recv().await {
        let resp = match deck_of_cards::draw_cards(client.clone(), deck_id, cards) {
            Ok(req) => req.await.map_err(Into::into),
            Err(err) => Err(ActionError::CantBeZeroError(err))
        };
        response.send(resp).unwrap();
    }
}
