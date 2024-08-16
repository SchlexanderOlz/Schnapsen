use core::fmt;
use std::borrow::BorrowMut;
use std::cell::Ref;
use std::cell::RefCell;
use std::ops::Deref;

use models::{Card, Player};
use rand::prelude::*;
use rand::thread_rng;

pub mod models;

#[derive(Debug)]
pub enum PlayerError {
    CantPlay40,
    CantPlay20,
    CantTakeCardStackEmpty,
    CantPlayCard(Card),
    PlayerNotActive,
}

impl PlayerError {
    pub const fn get_message(&self) -> &str {
        match self {
            PlayerError::CantPlay40 => "Player can't play 40 right now",
            PlayerError::CantPlay20 => "Player can't play 20 right now",
            PlayerError::CantTakeCardStackEmpty => "Player can't take card from empty stack",
            PlayerError::CantPlayCard(_) => "Player can't play this card",
            PlayerError::PlayerNotActive => "Player is not active",
        }
    }
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlayerError: {}", self.get_message())
    }
}
impl std::error::Error for PlayerError {}

pub enum SchnapsenDuoActions {
    // TODO: Continue here with finding all events which happen in the game. Then add handlers
}

pub struct SchnapsenDuo<'a> {
    players: [RefCell<Player>; 2],
    deck: Vec<Card>,
    active: Option<&'a RefCell<Player>>,
    trump: Option<models::CardSuit>,
    stack: Vec<Card>,
}

impl<'a> SchnapsenDuo<'a> {
    pub fn new(player_ids: &[String; 2]) -> SchnapsenDuo {
        let deck = SchnapsenDuo::populate_deck();
        let players = [
            RefCell::new(Player {
                id: player_ids[0].clone(),
                cards: Vec::new(),
                tricks: Vec::new(),
                announcement: None,
                points: 0,
            }),
            RefCell::new(Player {
                id: player_ids[1].clone(),
                cards: Vec::new(),
                tricks: Vec::new(),
                announcement: None,
                points: 0,
            }),
        ];

        SchnapsenDuo {
            active: None,
            players,
            deck: deck.into(),
            trump: None,
            stack: Vec::new(),
        }
    }

    #[inline]
    pub fn get_active_player(&'a self) -> Option<&RefCell<Player>> {
        self.active
    }

    pub fn cutt_deck(&mut self, mut cards_to_take: usize) -> Result<(), PlayerError> {
        if cards_to_take > self.deck.len() {
            cards_to_take = self.deck.len();
        }
        let (back, front) = self.deck.split_at(cards_to_take);
        self.deck = front.into_iter().chain(back.into_iter()).cloned().collect();
        Ok(())
    }

    pub fn take_top_card(&mut self) -> Result<Card, PlayerError> {
        if self.deck.is_empty() {
            return Err(PlayerError::CantTakeCardStackEmpty);
        }
        self.active
            .unwrap()
            .borrow_mut()
            .cards
            .push(self.deck.pop().unwrap());
        Ok(self.deck.pop().unwrap())
    }

    #[inline]
    pub fn get_player_with_greates_card(&'a self) -> Option<&RefCell<Player>> {
        self.players.iter().max_by_key(|player| {
            assert!(player.borrow().cards.len() == 1);
            player.borrow().cards[0].value.clone() as u8
        })
    }

    #[inline]
    pub fn set_start_player(&'a mut self, player: &'a Player) {
        self.active = self
            .players
            .iter()
            .find(|p| std::ptr::eq(p.borrow().deref(), player));
    }

    pub fn play_card(&mut self, card: Card) -> Result<(), PlayerError> {
        if self.active.is_none() {
            return Err(PlayerError::PlayerNotActive);
        }
        let active = self.active.unwrap();

        if !active.borrow().cards.contains(&card) {
            return Err(PlayerError::CantPlayCard(card));
        }

        active.borrow_mut().cards.retain(|c| c != &card);
        self.stack.push(card);

        let enemy = self
            .players
            .iter()
            .find(|p| !std::ptr::eq(p.borrow().deref(), active.borrow().deref()))
            .unwrap();
        if self.stack.len() == 2 {
            if self.stack.last().unwrap().suit != self.stack.first().unwrap().suit {
                enemy
                    .borrow_mut()
                    .tricks
                    .push((self.stack.pop().unwrap(), self.stack.pop().unwrap()));
            } else if self.stack.last().unwrap().value.clone() as u8
                > self.stack.first().unwrap().value.clone() as u8
            {
                active
                    .borrow_mut()
                    .tricks
                    .push((self.stack.pop().unwrap(), self.stack.pop().unwrap()));
            } else {
                active
                    .borrow_mut()
                    .tricks
                    .push((self.stack.pop().unwrap(), self.stack.pop().unwrap()));
            }

            let points = self
                .players
                .iter()
                .map(|player| {
                    player
                        .borrow()
                        .cards
                        .iter()
                        .fold(0, |acc, card| acc + card.value.clone() as u8)
                })
                .zip(self.players.iter());

            let (max_points, winner) = points
                .clone()
                .max_by_key(|(mut points, player)| {
                    if points > 0 {
                        if let Some(announcement) = &player.borrow().announcement {
                            points += announcement.announcement_type.clone() as u8;
                        }
                    }
                    points
                })
                .unwrap();

            let (min_points, loser) = points.min_by_key(|(points, _)| *points).unwrap();

            if max_points < 66 {
                return Ok(());
            }
            // TODO: Someone won the game here

            if min_points == 0 {
                winner.borrow_mut().points += 3;
            } else if min_points >= 33 {
                winner.borrow_mut().points += 2;
            } else {
                winner.borrow_mut().points += 1;
            }
            winner.borrow_mut().reset();
            loser.borrow_mut().reset();
        }

        Ok(())
    }

    pub fn announce_40(&mut self) -> Result<(), PlayerError> {
        let cards_to_announce = self.can_announce_20();
        if cards_to_announce.is_none() {
            return Err(PlayerError::CantPlay40);
        }
        let cards_to_announce = cards_to_announce.unwrap();

        if cards_to_announce.first().unwrap().suit != self.trump.as_ref().unwrap().clone() {
            return Err(PlayerError::CantPlay40);
        }

        self.active.unwrap().borrow_mut().announcement = Some(models::Announcement {
            cards: cards_to_announce,
            announcement_type: models::AnnounceType::Forty,
        });

        Ok(())
    }

    pub fn announce_20(&mut self) -> Result<(), PlayerError> {
        let announce = self.can_announce_20();
        if announce.is_none() {
            return Err(PlayerError::CantPlay20);
        }
        let announce = announce.unwrap();

        self.active.unwrap().borrow_mut().announcement = Some(models::Announcement {
            cards: announce,
            announcement_type: models::AnnounceType::Twenty,
        });
        Ok(())
    }

    #[inline]
    // TODO: This function returns a fucking value because of some stupid fucking borrowing rules your mum invented. Change this to a fucking stupid refernce as soon as possible
    fn can_announce_20(&'a self) -> Option<[Card; 2]> {
        if self.active.is_none() {
            return None;
        }
        let player = self.active.unwrap();

        (0..4).find_map(move |suit: u8| {
            let borrow = player.borrow();
            let mut cards_iter = borrow.cards.iter().filter(|card| {
                card.suit == suit.into()
                    && (card.value == models::CardVal::Jack || card.value == models::CardVal::King)
            });
            if let (Some(card1), Some(card2)) = (cards_iter.next(), cards_iter.next()) {
                Some([card1.clone(), card2.clone()])
            } else {
                None
            }
        })
    }

    fn populate_deck() -> [Card; 20] {
        let mut deck = (0..4)
            .map(|suit: u8| {
                (7..12).map(move |value: u8| Card {
                    value: value.into(),
                    suit: suit.into(),
                })
            })
            .flatten()
            .collect::<Vec<Card>>();

        let mut rng = thread_rng();
        deck.shuffle(&mut rng);
        deck.try_into()
            .expect("Programming error. Populated deck is not of length 20")
    }
}
