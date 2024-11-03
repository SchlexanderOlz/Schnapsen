use std::hash::Hash;

use num_enum::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Card {
    pub value: CardVal,
    pub suit: CardSuit,
}

impl PartialOrd for Card {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.value.cmp(&other.value))
    }
}

impl Ord for Card {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CardVal {
    Ten = 10,
    Jack = 2,
    Queen = 3,
    King = 4,
    #[default]
    Ace = 11,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum CardSuit {
    #[default]
    Hearts = 0,
    Diamonds = 1,
    Clubs = 2,
    Spades = 3,
}

#[derive(Clone)]
pub struct Player {
    pub id: String,
    pub cards: Vec<Card>,
    pub playable_cards: Vec<Card>,
    pub tricks: Vec<[Card; 2]>,
    pub announcements: Vec<Announcement>,
    pub announcable: Vec<Announcement>,
    pub possible_trump_swap: Option<Card>,
    pub points: u8,
}

impl Player {
    pub fn reset(&mut self) {
        self.cards.clear();
        self.tricks.clear();
        self.announcements.clear();
        self.announcable.clear();
    }

    pub fn new(id: String) -> Self {
        Player {
            id,
            cards: Vec::new(),
            playable_cards: Vec::new(),
            tricks: Vec::new(),
            announcements: Vec::new(),
            announcable: Vec::new(),
            points: 0,
            possible_trump_swap: None,
        }
    }

    pub fn has_announced(&self, mut cards: [Card; 2]) -> bool {
        cards.sort();
        let announced = self
            .announcements
            .iter()
            .map(|x| {
                let mut a = x.cards.clone();
                a.sort();
                a
            })
            .collect::<Vec<_>>();
        announced.into_iter().any(|x| x == cards)
    }

    pub fn has_announcable(&self, announcement: &Announcement) -> bool {
        has_announcable(&self.announcable, announcement)
    }
}

pub fn has_announcable(data: &[Announcement], check: &Announcement) -> bool {
    data.iter().any(|proposed| 
        proposed.announce_type == check.announce_type
            && proposed.cards.first().unwrap().suit == check.cards.first().unwrap().suit
    )
}

pub fn contains_card_comb(data: &[[Card; 2]], mut check: [Card; 2]) -> bool {
    check.sort();
    let announced = data
        .iter()
        .map(|x| {
            let mut clone = x.clone();
            clone.sort();
            clone
        })
        .collect::<Vec<_>>();
    announced.into_iter().any(|x| x == check)
}

impl Hash for Player {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.id.as_bytes());
    }
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub cards: [Card; 2],
    pub announce_type: AnnounceType,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum AnnounceType {
    Forty = 40,
    #[default]
    Twenty = 20,
}
