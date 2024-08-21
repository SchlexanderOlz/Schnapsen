use std::{cell::RefCell, default, hash::Hash};

use num_enum::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Card {
    pub value: CardVal,
    pub suit: CardSuit,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum CardVal {
    Ten = 10,
    Jack = 2,
    Queen = 3,
    King = 4,
    #[default]
    Ace = 11
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum CardSuit {
    #[default]
    Hearts = 0,
    Diamonds = 1,
    Clubs = 2,
    Spades = 3
}

#[derive(Clone)]
pub struct Player {
    pub id: String,
    pub cards: Vec<Card>,
    pub playable_cards: Vec<Card>,
    pub tricks: Vec<[Card; 2]>,
    pub announcement: Option<Announcement>,
    pub points: u8,
}

impl Player {
    pub fn reset(&mut self) {
        self.cards.clear();
        self.tricks.clear();
        self.announcement = None;
    }

    pub fn new(id: String) -> Self {
        Player {
            id,
            cards: Vec::new(),
            playable_cards: Vec::new(),
            tricks: Vec::new(),
            announcement: None,
            points: 0,
        }
    }
}

impl Hash for Player {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.id.as_bytes());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Announcement {
    pub cards: [Card; 2],
    pub announcement_type: AnnounceType,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum AnnounceType {
    Forty = 40,
    #[default]
    Twenty = 20,
}
