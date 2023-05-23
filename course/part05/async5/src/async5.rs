#![allow(dead_code, unused_imports)]

use std::{fmt::{Debug, Display}, future::Future, pin::pin, sync::Arc, time::Duration};
use std::fmt::Formatter;

use anyhow::Context;
use reqwest::{Client, ClientBuilder};
use tokio::select;
use url::Url;

// http requests + awaiting multiple tasks

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ClientBuilder::new().build()?;

    let deck_id = new_deck(client.clone()).await?;

    // linearly
    let drawn_cards1 = draw_cards(client.clone(), deck_id, 2).await?;
    let drawn_cards2 = draw_cards(client.clone(), deck_id, 3).await?;

    // concurrently, same task
    // let drawn_cards1 = draw_cards(client.clone(), deck_id, 2);
    // let drawn_cards2 = draw_cards(client.clone(), deck_id, 3);
    // let (drawn_cards1, drawn_cards2) = tokio::try_join!(drawn_cards1, drawn_cards2)?;
    // let (cards1_res, cards2_res) = tokio::join!(drawn_cards1, drawn_cards2);
    // let drawn_cards1 = cards1_res?;
    // let drawn_cards2 = cards2_res?;

    // concurrently, different tasks
    // let drawn_cards1 = draw_cards(client.clone(), deck_id, 2);
    // let drawn_cards2 = draw_cards(client.clone(), deck_id, 3);
    // let (cards1_res, cards2_res) =
    //     tokio::try_join!(tokio::spawn(drawn_cards1), tokio::spawn(drawn_cards2)).context("a task panicked")?;
    // let drawn_cards1 = cards1_res?;
    // let drawn_cards2 = cards2_res?;

    println!("total retrieved cards :{}", drawn_cards1.cards.len() + drawn_cards2.cards.len());

    let pretty = toml::to_string_pretty(&drawn_cards1).unwrap();

    println!("{pretty}");

    /* selecting on futures */

    select! {
        drawn_cards = draw_cards(client.clone(), deck_id, 4) => {
            println!("selected cards: {:#?}", drawn_cards?.cards);
        },
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            println!("timeout in select hit");
        }
    }

    let mut cards = pin!(draw_cards(client.clone(), deck_id, 2));
    let mut ticker = tokio::time::interval(Duration::from_millis(10));
    let drawn_cards: DrawnCardsInfo = loop {
        select! {
            drawn_cards = cards.as_mut() => {
                break drawn_cards;
            }
            _ = ticker.tick() => {
                println!("i am still waiting...");
            }
        }
    }?;

    println!("cards from looped select: {:#?}", drawn_cards.cards);

    // select futures must be safe to only partially poll - .writeAll isn't

    Ok(())
}

async fn new_deck(client: Client) -> Result<DeckID, reqwest::Error> {
    let deck_info: DeckInfo = client
        .get("https://deckofcardsapi.com/api/deck/new/shuffle/?deck_count=1")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(deck_info.deck_id)
}

// this function runs its first part sync, then returns an async block
// this is useful especially when there's some immediately failable setup
fn draw_cards(
    client: Client,
    deck_id: DeckID,
    n: u8,
) -> impl Future<Output = Result<DrawnCardsInfo, reqwest::Error>> {
    let req = client
        .get(format!(
            "https://deckofcardsapi.com/api/deck/{deck_id}/draw/?count={n}"
        ))
        .send();

    async move { req.await?.json().await }
}

#[derive(Copy, Clone)]
struct DeckID([u8; 12]);

impl Debug for DeckID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeckID({})", self.as_ref())
    }
}

impl Display for DeckID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for DeckID {
    fn as_ref(&self) -> &str {
        std::str::from_utf8(&self.0)
            .expect("id should be valid utf-8 string as it consists of ascii chars")
    }
}

impl serde::ser::Serialize for DeckID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> serde::Deserialize<'de> for DeckID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DeckIDVisitor;

        impl<'vi> serde::de::Visitor<'vi> for DeckIDVisitor {
            type Value = DeckID;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "a 12 char ascii string representing an ID")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if !v.is_ascii() {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(v),
                        &self,
                    ));
                }

                let mut res = [0; 12];
                let mut chars = v.chars();

                for (i, b) in res.iter_mut().enumerate() {
                    let Some(c) = chars.next() else {
                        return Err(serde::de::Error::invalid_length(i, &self));
                    };

                    let Ok(byte) = c.try_into() else {
                        return Err(serde::de::Error::invalid_value(serde::de::Unexpected::Char(c), &self));
                    };

                    *b = byte;
                }

                if chars.next().is_some() {
                    return Err(serde::de::Error::invalid_length(13 + chars.count(), &self));
                }

                Ok(DeckID(res))
            }
        }

        deserializer.deserialize_str(DeckIDVisitor {})
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct DeckInfo {
    success: bool,
    deck_id: DeckID,
    shuffled: bool,
    remaining: u8,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct DrawnCardsInfo {
    success: bool,
    deck_id: DeckID,
    cards: Box<[Card]>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Card {
    code: Code,
    image: Url,
    images: Images,
    value: Value,
    suit: Suit,
}

#[derive(Debug)]
struct Code {
    value: Value,
    suit: Suit,
}

// a manual implementation of Serialize that serializes to a 2 char string
impl serde::ser::Serialize for Code {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self.value {
            Value::Ace => b'A',
            Value::Value2 => b'2',
            Value::Value3 => b'3',
            Value::Value4 => b'4',
            Value::Value5 => b'5',
            Value::Value6 => b'6',
            Value::Value7 => b'7',
            Value::Value8 => b'8',
            Value::Value9 => b'9',
            Value::Value10 => b'0',
            Value::Jack => b'J',
            Value::Queen => b'Q',
            Value::King => b'K',
        };
        let suit = match self.suit {
            Suit::Clubs => b'C',
            Suit::Diamonds => b'D',
            Suit::Spades => b'S',
            Suit::Hearts => b'H',
        };
        let s = [value, suit];
        let s = std::str::from_utf8(&s).expect("manually built string should be valid utf-8");
        serializer.serialize_str(s)
    }
}

// a manual implementation of Deserialize that deserializes from a 2 char string to a struct of enums
impl<'de> serde::Deserialize<'de> for Code {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CodeVisitor;

        impl<'vi> serde::de::Visitor<'vi> for CodeVisitor {
            type Value = Code;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "a string of 2 chars representing a card")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mut chars = v.chars();

                let Some(value) = chars.next() else {
                    return Err(serde::de::Error::invalid_length(0, &self));
                };
                let value = match value {
                    'A' => Value::Ace,
                    '2' => Value::Value2,
                    '3' => Value::Value3,
                    '4' => Value::Value4,
                    '5' => Value::Value5,
                    '6' => Value::Value6,
                    '7' => Value::Value7,
                    '8' => Value::Value8,
                    '9' => Value::Value9,
                    '0' => Value::Value10,
                    'J' => Value::Jack,
                    'Q' => Value::Queen,
                    'K' => Value::King,
                    c => {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Char(c),
                            &self,
                        ))
                    }
                };

                let Some(suit) = chars.next() else {
                    return Err(serde::de::Error::invalid_length(1, &self));
                };
                let suit = match suit {
                    'C' => Suit::Clubs,
                    'D' => Suit::Diamonds,
                    'H' => Suit::Hearts,
                    'S' => Suit::Spades,
                    c => {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Char(c),
                            &self,
                        ))
                    }
                };

                if chars.next().is_some() {
                    return Err(serde::de::Error::invalid_length(3 + chars.count(), &self));
                };

                Ok(Code { value, suit })
            }
        }

        deserializer.deserialize_str(CodeVisitor {})
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Images {
    svg: Url,
    png: Url,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum Value {
    Ace,

    #[serde(rename = "2")]
    Value2,

    #[serde(rename = "3")]
    Value3,

    #[serde(rename = "4")]
    Value4,

    #[serde(rename = "5")]
    Value5,

    #[serde(rename = "6")]
    Value6,

    #[serde(rename = "7")]
    Value7,

    #[serde(rename = "8")]
    Value8,

    #[serde(rename = "9")]
    Value9,

    #[serde(rename = "10")]
    Value10,

    Jack,

    Queen,

    King,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum Suit {
    Clubs,

    Diamonds,

    Spades,

    Hearts,
}
