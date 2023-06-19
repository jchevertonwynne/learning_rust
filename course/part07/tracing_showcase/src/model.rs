use std::fmt::{Debug, Formatter, Display};

use rand::seq::SliceRandom;
use url::Url;

use crate::grpc::proto;


#[derive(Copy, Clone)]
pub struct DeckID([u8; 12]);

impl DeckID {
    pub fn random() -> Self {
        let chars = b"0123456789qwertyuiopasdfghjklzxcvbnm";
        DeckID(std::array::from_fn(|_| {
            *chars
                .choose(&mut rand::thread_rng())
                .expect("slice is non empty")
        }))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("deck ID must be 12 lowercase letters or numbers")]
pub struct DeckIDParseError;

impl<'a> TryFrom<&'a str> for DeckID {
    type Error = DeckIDParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        if value.len() != 12 {
            return Err(DeckIDParseError);
        }

        if !value.is_ascii()
            || !value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_alphanumeric())
        {
            return Err(DeckIDParseError);
        }

        Ok(DeckID(std::array::from_fn(|i| value.as_bytes()[i])))
    }
}

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
                DeckID::try_from(v).map_err(|_| {
                    serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &self)
                })
            }
        }

        deserializer.deserialize_str(DeckIDVisitor {})
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct DeckInfo {
    pub success: bool,
    pub deck_id: DeckID,
    pub shuffled: bool,
    pub remaining: usize,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct DrawnCardsInfo {
    pub success: bool,
    pub deck_id: DeckID,
    pub cards: Box<[Card]>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Card {
    pub code: Code,
    pub image: Url,
    pub images: Images,
    pub value: Value,
    pub suit: Suit,
}

impl<'a> From<&'a Card> for proto::Card {
    fn from(card: &'a Card) -> Self {
        let value: proto::Value = (&card.value).into();
        let suit: proto::Suit = (&card.suit).into();
        Self {
            value: value.into(),
            suit: suit.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Code {
    pub value: Value,
    pub suit: Suit,
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

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Images {
    pub svg: Url,
    pub png: Url,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy, strum_macros::EnumIter)]
#[serde(rename_all = "UPPERCASE")]
pub enum Value {
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

impl<'a> From<&'a Value> for proto::Value {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Ace => Self::Ace,
            Value::Value2 => Self::Value2,
            Value::Value3 => Self::Value3,
            Value::Value4 => Self::Value4,
            Value::Value5 => Self::Value5,
            Value::Value6 => Self::Value6,
            Value::Value7 => Self::Value7,
            Value::Value8 => Self::Value8,
            Value::Value9 => Self::Value9,
            Value::Value10 => Self::Value10,
            Value::Jack => Self::Jack,
            Value::Queen => Self::Queen,
            Value::King => Self::King,
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy, strum_macros::EnumIter)]
#[serde(rename_all = "UPPERCASE")]
pub enum Suit {
    Clubs,

    Diamonds,

    Spades,

    Hearts,
}

impl<'a> From<&'a Suit> for proto::Suit {
    fn from(value: &'a Suit) -> Self {
        match value {
            Suit::Clubs => Self::Club,
            Suit::Diamonds => Self::Diamond,
            Suit::Spades => Self::Spade,
            Suit::Hearts => Self::Heart,
        }
    }
}
