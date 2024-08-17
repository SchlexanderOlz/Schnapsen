use core::fmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Index;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::panicking;

use models::Announcement;
use models::{Card, Player};
use rand::prelude::*;
use rand::thread_rng;
use serde::Deserialize;
use serde::Serialize;

pub mod models;

#[derive(Debug)]
pub enum PlayerError {
    CantPlay40,
    CantPlay20,
    CantTakeCardDeckEmpty,
    CantPlayCard(Card),
    PlayerNotActive,
    CardNotTrump,
    CantTakeCardRoundNotFinished,
    CantDistributeCardsNoPlayerActive,
    CantTakeAllDeckCards,
    NotAllPlayersHaveTakenCards,
    CantSetActivePlayer,
    CantSwapTrump,
}

impl PlayerError {
    pub const fn get_message(&self) -> &str {
        match self {
            PlayerError::CantPlay40 => "Player can't play 40 right now",
            PlayerError::CantPlay20 => "Player can't play 20 right now",
            PlayerError::CantTakeCardDeckEmpty => "Player can't take card from empty card deck",
            PlayerError::CantPlayCard(_) => "Player can't play this card",
            PlayerError::PlayerNotActive => "Player is not active",
            PlayerError::CardNotTrump => "Player can't play card because it's not trump",
            PlayerError::CantTakeCardRoundNotFinished => {
                "Player can't take card because round is not finished"
            },
            PlayerError::CantDistributeCardsNoPlayerActive => {
                "Can't distribute cards because no player is active"
            },
            PlayerError::CantTakeAllDeckCards => {
                "Can't take so much deck cards because there are not enough cards or the opponent would have any cards left to pick"
            },
            PlayerError::NotAllPlayersHaveTakenCards => {
                "Not all players have taken cards"
            },
            PlayerError::CantSetActivePlayer => {
                "Can't set active player because some player is already active"
            },
            PlayerError::CantSwapTrump => {
                "Can't swap trump because player has no jack of trump in his hand"
            }
        }
    }
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlayerError: {}", self.get_message())
    }
}
impl std::error::Error for PlayerError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchnapsenDuoPrivateEvent {
    // TODO: Continue here with finding all events which happen in the game. Then add handlers
    CanAnnounce(Announcement),
    CardAvailabe(Card),
    CardUnavailabe(Card),
    Trick([Card; 2]),
    Result(u8),
    TrumpChange(Card),
    Active,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchnapsenDuoPublicEvent {
    PlayCard(Card),
    Trick(String, [Card; 2]),
    Announce(Announcement),
    Result(String, u8),
    FinalResult(String),
    CloseTalon,
    TrumpChange(Card),
    Active(String),
    Inactive(String),
    DeckCardCount(usize),
    ReceiveCard(String),
}

type FPub = Arc<dyn Fn(SchnapsenDuoPublicEvent) -> () + Send + Sync + 'static>;
type FPriv = Arc<dyn Fn(SchnapsenDuoPrivateEvent) -> () + Send + Sync + 'static>;

pub struct SchnapsenDuo {
    players: [Rc<RefCell<Player>>; 2],
    deck: Vec<Card>,
    active: Option<Rc<RefCell<Player>>>,
    trump: Option<models::Card>,
    stack: Vec<Card>,
    pub_callbacks: Vec<FPub>,
    priv_callbacks: HashMap<String, Vec<FPriv>>,
}

unsafe impl Send for SchnapsenDuo {}
unsafe impl Sync for SchnapsenDuo {}

impl SchnapsenDuo {
    pub fn new(player_ids: &[String; 2]) -> Self {
        let deck = Self::populate_deck();
        let players = [
            Rc::new(RefCell::new(Player {
                id: player_ids[0].clone(),
                cards: Vec::new(),
                tricks: Vec::new(),
                announcement: None,
                points: 0,
            })),
            Rc::new(RefCell::new(Player {
                id: player_ids[1].clone(),
                cards: Vec::new(),
                tricks: Vec::new(),
                announcement: None,
                points: 0,
            })),
        ];

        Self {
            active: None,
            players,
            deck: deck.into(),
            trump: None,
            stack: Vec::new(),
            pub_callbacks: Vec::new(),
            priv_callbacks: HashMap::new(),
        }
    }

    #[inline]
    pub fn on_priv_event(&mut self, player: Rc<RefCell<Player>>, callback: FPriv) {
        self.priv_callbacks
            .entry(player.borrow().id.clone())
            .or_insert_with(Vec::new)
            .push(callback);
    }

    #[inline]
    pub fn on_pub_event(&mut self, callback: FPub) {
        self.pub_callbacks.push(callback);
    }
    
    pub fn get_player(&self, player_id: &str) -> Option<Rc<RefCell<Player>>> {
        self.players
            .iter()
            .find(|player| player.borrow().id == player_id)
            .cloned()
    }

    #[inline]
    pub fn get_active_player(&self) -> Option<Rc<RefCell<Player>>> {
        self.active.clone()
    }

    #[inline]
    pub fn get_non_active_player(&self) -> Option<Rc<RefCell<Player>>> {
        self.players
            .iter()
            .find(|player| Rc::ptr_eq(*player, self.active.as_ref().unwrap()))
            .cloned()
    }

    pub fn cutt_deck(&mut self, player: Rc<RefCell<Player>>, mut cards_to_take: usize) -> Result<(), PlayerError> {
        if Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Err(PlayerError::CantTakeCardRoundNotFinished);
        }
        if cards_to_take > self.deck.len() {
            cards_to_take = self.deck.len();
        }
        let (back, front) = self.deck.split_at(cards_to_take);
        self.deck = front.into_iter().chain(back.into_iter()).cloned().collect();
        Ok(())
    }

    pub fn draw_card_after_trick(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Err(PlayerError::CantTakeCardRoundNotFinished);
        }
        if !self.stack.is_empty() {
            return Err(PlayerError::CantTakeCardRoundNotFinished);
        }

        if player.borrow().cards.len() < 5 {
            return Err(PlayerError::CantTakeCardRoundNotFinished);
        }
        self.draw_card(player)?;
        self.swap_to(self.get_non_active_player().unwrap());
        Ok(())
    }

    pub fn draw_card(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if self.deck.is_empty() {
            return Err(PlayerError::CantTakeCardDeckEmpty);
        }
        player
            .borrow_mut()
            .cards
            .push(self.deck.pop().unwrap());
    
        self.notify_pub(SchnapsenDuoPublicEvent::DeckCardCount(self.deck.len()));

        let card = self.deck.pop().unwrap();
        self.notify_priv(
            player.borrow().id.clone(),
            SchnapsenDuoPrivateEvent::CardAvailabe(card.clone()),
        );
        self.notify_pub(SchnapsenDuoPublicEvent::ReceiveCard(
            player.borrow().id.clone(),
        ));
        player.borrow_mut().cards.push(card);
        self.run_announce_checks(player.clone());
        self.run_swap_trump_check(player);

        Ok(())
    }

    #[inline]
    pub fn take_cards_til(
        &mut self,
        player: Rc<RefCell<Player>>,
        idx: usize,
    ) -> Result<(), PlayerError> {
        if idx >= self.deck.len() - 1 {
            return Err(PlayerError::CantTakeAllDeckCards);
        }
        if self.deck.is_empty() {
            return Err(PlayerError::CantTakeCardDeckEmpty);
        }

        for card in self.deck.drain(..idx).collect::<Vec<_>>() {
            player.borrow_mut().cards.push(card.clone());
            self.notify_priv(player.borrow().id.clone(), SchnapsenDuoPrivateEvent::CardAvailabe(card));
            self.notify_pub(SchnapsenDuoPublicEvent::ReceiveCard(player.borrow().id.clone()));
        }

        self.notify_pub(SchnapsenDuoPublicEvent::DeckCardCount(self.deck.len()));
        Ok(())
    }

    #[inline]
    pub fn get_player_with_greatest_card(&self) -> Result<Rc<RefCell<Player>>, PlayerError> {
        if self
            .players
            .iter()
            .any(|player| player.borrow().cards.is_empty())
        {
            return Err(PlayerError::NotAllPlayersHaveTakenCards);
        }
        Ok(self
            .players
            .iter()
            .max_by_key(|player| player.borrow().cards.last().unwrap().value.clone() as u8)
            .unwrap()
            .clone())
    }

    pub fn recreate_deck(&mut self) {
        self.deck = Self::populate_deck().into();
        self.players.iter().for_each(|player| {
            player.as_ref().borrow_mut().reset();
        });
    }

    pub fn distribute_cards(&mut self) -> Result<(), PlayerError> {
        if self.active.is_none() {
            return Err(PlayerError::CantDistributeCardsNoPlayerActive);
        }
        let mut player_order: Vec<Rc<RefCell<Player>>> = self.players.iter().cloned().collect();

        if !Rc::ptr_eq(self.players.index(0), &self.active.clone().unwrap()) {
            player_order = self.players.iter().cloned().rev().collect();
        }

        for player in player_order.clone() {
            for _ in 0..3 {
                self.do_cards(player.clone());
            }
        }
        let trump = self.deck.pop().unwrap();
        self.notify_pub(SchnapsenDuoPublicEvent::TrumpChange(trump.clone()));
        let _ = self.trump.insert(trump);

        for player in player_order {
            for _ in 0..2 {
                self.do_cards(player.clone());
            }
        }
        Ok(())
    }

    #[inline]
    pub fn set_active_player(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if self.active.is_some() {
            return Err(PlayerError::CantSetActivePlayer);
        }
        self.swap_to(player);
        Ok(())
    }

    pub fn play_card(&mut self, player: Rc<RefCell<Player>>, card: Card) -> Result<(), PlayerError> {
        if self.active.is_none() || !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Err(PlayerError::PlayerNotActive);
        }

        if !player.borrow().cards.contains(&card) {
            return Err(PlayerError::CantPlayCard(card));
        }

        if card.suit != self.trump.as_ref().unwrap().suit {
            if  player
                .borrow()
                .cards
                .iter()
                .any(|c| c.suit == self.trump.as_ref().unwrap().suit)
            {
                return Err(PlayerError::CardNotTrump);
            }
        }

        player
            .borrow_mut()
            .cards
            .retain(|c| c != &card);

        self.notify_pub(SchnapsenDuoPublicEvent::PlayCard(card.clone()));
        self.notify_priv(
            player.borrow().id.clone(),
            SchnapsenDuoPrivateEvent::CardUnavailabe(card.clone()),
        );
        self.stack.push(card);

        if self.stack.len() == 2 {
            self.handle_trick()?;
        } else {
            self.swap_to(self.get_non_active_player().unwrap());
        }
        self.run_after_move_checks();
        Ok(())
    }

    pub fn swap_trump(&mut self, player: Rc<RefCell<Player>>, card: Card) -> Result<(), PlayerError> {
        if let Some(pos) = self.can_swap_trump(player.clone()) {
            {
                if player.borrow().cards[pos] != card {
                    return Err(PlayerError::CantSwapTrump);
                }
                let mut borrow = player.borrow_mut();
                let other = borrow.cards.remove(pos);

                let trump = self.trump.take().unwrap();

                borrow.cards.push(trump.clone());
                self.trump = Some(other.clone());

                self.notify_priv(
                    borrow.id.clone(),
                    SchnapsenDuoPrivateEvent::CardAvailabe(trump),
                );
                self.notify_priv(
                    borrow.id.clone(),
                    SchnapsenDuoPrivateEvent::CardUnavailabe(other),
                );
            }

            self.notify_pub(SchnapsenDuoPublicEvent::TrumpChange(
                self.trump.as_ref().unwrap().clone(),
            ));
            return Ok(());
        }
        Err(PlayerError::CantSwapTrump)
    }

    pub fn announce_40(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        let cards_to_announce = self.can_announce_40(player);
        if cards_to_announce.is_none() {
            return Err(PlayerError::CantPlay40);
        }

        let announcement = models::Announcement {
            cards: cards_to_announce.unwrap(),
            announcement_type: models::AnnounceType::Forty,
        };

        self.active.as_deref().unwrap().borrow_mut().announcement = Some(announcement.clone());
        self.notify_pub(SchnapsenDuoPublicEvent::Announce(announcement.clone()));
        self.run_after_move_checks();
        Ok(())
    }

    pub fn announce_20(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        let announce = self.can_announce_20(player);
        if announce.is_none() {
            return Err(PlayerError::CantPlay20);
        }
        let announce = announce.unwrap();

        let announcement = models::Announcement {
            cards: announce,
            announcement_type: models::AnnounceType::Forty,
        };

        self.active.as_deref().unwrap().borrow_mut().announcement = Some(announcement.clone());
        self.notify_pub(SchnapsenDuoPublicEvent::Announce(announcement.clone()));
        self.run_after_move_checks();
        Ok(())
    }

    #[inline]
    // TODO: This function returns a fucking value because of some stupid fucking borrowing rules your mum invented. Change this to a fucking stupid refernce as soon as possible
    fn can_announce_20(&self, player: Rc<RefCell<Player>>) -> Option<[Card; 2]> {
        if self.active.is_none() || !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return None;
        }
        let player = self.active.as_deref().unwrap();

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

    fn can_announce_40(&self, player: Rc<RefCell<Player>>) -> Option<[Card; 2]> {
        let cards_to_announce = self.can_announce_20(player.clone());
        if cards_to_announce.is_none() {
            return None;
        }
        let cards_to_announce = cards_to_announce.unwrap();

        if cards_to_announce.first().unwrap().suit != self.trump.as_ref().unwrap().suit {
            return None;
        }
        Some(cards_to_announce)
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

    fn notify_pub(&self, event: SchnapsenDuoPublicEvent) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();
        for callback in self.pub_callbacks.iter().cloned() {
            let clone = event.clone();
            handles.push(tokio::task::spawn(async move { callback(clone) }));
        }
        handles
    }

    fn notify_priv(
        &self,
        user_id: String,
        event: SchnapsenDuoPrivateEvent,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();
        if let Some(callbacks) = self.priv_callbacks.get(&user_id).cloned() {
            for callback in callbacks {
                let clone = event.clone();
                handles.push(tokio::task::spawn(async move { callback(clone) }));
            }
        }
        handles
    }

    fn run_announce_checks(&self, player: Rc<RefCell<Player>>) {
        let id = player.borrow().id.clone();
        if let Some(announcement) = self.can_announce_20(player.clone()) {
            let announcement = Announcement {
                cards: announcement,
                announcement_type: models::AnnounceType::Twenty,
            };
            self.notify_priv(
                id.clone(),
                SchnapsenDuoPrivateEvent::CanAnnounce(announcement),
            );
        }

        if let Some(announcement) = self.can_announce_40(player) {
            let announcement = Announcement {
                cards: announcement,
                announcement_type: models::AnnounceType::Forty,
            };
            self.notify_priv(id, SchnapsenDuoPrivateEvent::CanAnnounce(announcement));
        }
    }

    fn run_swap_trump_check(&self, player: Rc<RefCell<Player>>) {
        if self.can_swap_trump(player.clone()).is_some() {
            self.notify_priv(
                player.borrow().id.clone(),
                SchnapsenDuoPrivateEvent::TrumpChange(self.trump.as_ref().unwrap().clone()),
            );
        }
    }

    fn run_after_move_checks(&mut self) {}

    fn swap_to(&mut self, player: Rc<RefCell<Player>>) {
        if Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return;
        }
        let user_id = self.active.as_deref().unwrap().borrow().id.clone();
        self.notify_pub(SchnapsenDuoPublicEvent::Inactive(user_id.clone()));
        self.notify_priv(user_id, SchnapsenDuoPrivateEvent::Inactive);

        let _ = self.active.insert(player);

        let user_id = self.active.as_deref().unwrap().borrow().id.clone();
        self.notify_pub(SchnapsenDuoPublicEvent::Active(user_id.clone()));
        self.notify_priv(user_id, SchnapsenDuoPrivateEvent::Active);
    }

    fn handle_trick(&mut self) -> Result<(), PlayerError> {
        let won = {
            let enemy = self.get_non_active_player().unwrap();

            let active_is_trump =
                self.stack.first().unwrap().suit == self.trump.as_ref().unwrap().suit;
            let enemy_is_trump =
                self.stack.last().unwrap().suit == self.trump.as_ref().unwrap().suit;

            if !active_is_trump && enemy_is_trump {
                enemy.clone()
            } else if active_is_trump && !enemy_is_trump {
                self.active.clone().unwrap()
            } else if self.stack.last().unwrap().value.clone() as u8
                > self.stack.first().unwrap().value.clone() as u8
            {
                self.active.clone().unwrap()
            } else {
                enemy.clone()
            }
        };

        self.swap_to(won.clone());
        self.notify_pub(SchnapsenDuoPublicEvent::Trick(
            won.as_ref().borrow().id.clone(),
            self.stack.clone().try_into().unwrap(),
        ));
        self.notify_priv(
            won.borrow().id.clone(),
            SchnapsenDuoPrivateEvent::Trick(self.stack.clone().try_into().unwrap()),
        );

        won.as_ref()
            .borrow_mut()
            .tricks
            .push((self.stack.pop().unwrap(), self.stack.pop().unwrap()));

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

        let points;
        if min_points == 0 {
            points = 3;
        } else if min_points >= 33 {
            points = 2;
        } else {
            points = 1;
        }
        winner.as_ref().borrow_mut().points += points;
        self.notify_pub(SchnapsenDuoPublicEvent::Result(
            winner.borrow().id.clone(),
            points,
        ));

        winner.as_ref().borrow_mut().reset();
        loser.as_ref().borrow_mut().reset();

        let total_winner = self
            .players
            .iter()
            .find(|player| player.borrow().points >= 7);

        if total_winner.is_some() {
            self.notify_pub(SchnapsenDuoPublicEvent::FinalResult(
                total_winner.unwrap().borrow().id.clone(),
            ));
        }
        Ok(())
    }

    fn do_cards(&mut self, player: Rc<RefCell<Player>>) {
        let card = self.deck.pop().unwrap();
        self.notify_priv(
            player.as_ref().borrow().id.clone(),
            SchnapsenDuoPrivateEvent::CardAvailabe(card.clone()),
        );
        self.notify_pub(SchnapsenDuoPublicEvent::ReceiveCard(
            player.as_ref().borrow().id.clone(),
        ));
        player.as_ref().borrow_mut().cards.push(card);
    }

    fn can_swap_trump(&self, player: Rc<RefCell<Player>>) -> Option<usize> {
        if self.active.is_none() || !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return None;
        }

        self.active
            .as_ref()
            .unwrap()
            .borrow()
            .cards
            .iter()
            .position(|card| {
                card.suit == self.trump.as_ref().unwrap().suit
                    && card.value == models::CardVal::Jack
            })
    }
}
