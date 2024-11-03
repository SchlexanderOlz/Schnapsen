use core::fmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::mem::take;
use std::ops::Index;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::join_all;
use futures::FutureExt;
use models::{contains_card_comb, has_announcable, Announcement};
use models::{Card, Player};
use rand::prelude::*;
use rand::thread_rng;
use serde::Deserialize;
use serde::Serialize;

pub mod models;
mod scheduler;

#[derive(Debug)]
pub enum PlayerError {
    CantPlay40,
    CantPlay20,
    CantTakeCardDeckEmpty,
    CantPlayCard(Card),
    PlayerNotActive,
    CardNotTrump,
    CantTakeCardRoundNotFinished,
    NoPlayerActive,
    CantTakeAllDeckCards,
    NotAllPlayersHaveTakenCards,
    CantSetActivePlayer,
    CantSwapTrump,
    CantTakeCardPlayerNotActive,
    CantTakeCardHaveAlreadyFive,
    TalonAlreadyClosed,
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
            PlayerError::NoPlayerActive => {
                "No player is active"
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
            },
            PlayerError::CantTakeCardPlayerNotActive => {
                "Can't take card because player is not active"
            },
            PlayerError::CantTakeCardHaveAlreadyFive => {
                "Can't take card because player already has five cards"
            },
            PlayerError::TalonAlreadyClosed => {
                "Talon is already closed"
            }
        }
    }
}

pub struct PlayerPoint {
    player: Rc<RefCell<Player>>,
    points: u8,
}

pub struct CardComparisonResult {
    pub winner: PlayerPoint,
    pub loser: PlayerPoint,
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlayerError: {}", self.get_message())
    }
}
impl std::error::Error for PlayerError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PrivateEvent {
    CanAnnounce(Announcement),
    CannotAnnounce(Announcement),
    CardAvailabe(Card),
    CardUnavailabe(Card),
    CardPlayable(Card),
    CardNotPlayable(Card),
    TrumpChangePossible(Card),
    TrumpChangeImpossible(Card),
    AllowPlayCard,
    AllowDrawCard,
    AllowAnnounce,
    AllowCloseTalon,
}

// TODO: Alle user_ids are currently serialized as the write-tokens. This has to be changed. SECURITY RISK
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PublicEvent {
    Score {
        user_id: String,
        points: u8,
    },
    PlayCard {
        user_id: String,
        card: Card,
    },
    Trick {
        user_id: String,
        cards: [Card; 2],
    },
    Announce {
        user_id: String,
        announcement: Announcement,
    },
    Result {
        winner: String,
        points: u8,
        ranked: HashMap<String, u8>,
    },
    FinalResult {
        ranked: HashMap<String, u8>,
        winner: String,
    },
    CloseTalon {
        user_id: String,
    },
    TrumpChange(Option<Card>),
    // TrumpSwap(String, Card)
    Active {
        user_id: String,
    },
    Inactive {
        user_id: String,
    },
    DeckCardCount(usize),
    ReceiveCard {
        user_id: String,
    },
    FinishedDistribution,
}

type FPub = Arc<dyn Fn(PublicEvent) -> () + Send + Sync + 'static>;
type FPriv = Arc<dyn Fn(PrivateEvent) -> () + Send + Sync + 'static>;

pub struct SchnapsenDuo {
    players: [Rc<RefCell<Player>>; 2],
    deck: Vec<Card>,
    active: Option<Rc<RefCell<Player>>>,
    trump: Option<models::Card>,
    taken_trump: Option<(Rc<RefCell<Player>>, models::Card)>,
    closed_talon: Option<Rc<RefCell<Player>>>,
    stack: Vec<Card>,
    pub_callbacks: Vec<FPub>,
    priv_callbacks: HashMap<String, Vec<FPriv>>,
}

unsafe impl Send for SchnapsenDuo {}
unsafe impl Sync for SchnapsenDuo {}

impl Hash for SchnapsenDuo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.players
            .iter()
            .for_each(|player| player.borrow().hash(state));
        let now = chrono::Utc::now();
        state.write(now.timestamp().to_le_bytes().as_slice().into());
    }
}

impl SchnapsenDuo {
    pub fn new(player_ids: &[String; 2]) -> Self {
        let deck = Self::populate_deck();
        let players = [
            Rc::new(RefCell::new(Player::new(player_ids[0].clone()))),
            Rc::new(RefCell::new(Player::new(player_ids[1].clone()))),
        ];

        Self {
            active: None,
            players,
            deck: deck.into(),
            trump: None,
            stack: Vec::new(),
            pub_callbacks: Vec::new(),
            priv_callbacks: HashMap::new(),
            taken_trump: None,
            closed_talon: None,
        }
    }

    #[inline]
    pub fn on_priv_event(
        &mut self,
        player: Rc<RefCell<Player>>,
        callback: impl Fn(PrivateEvent) -> () + Send + Sync + 'static,
    ) {
        self.priv_callbacks
            .entry(player.borrow().id.clone())
            .or_insert_with(Vec::new)
            .push(Arc::new(callback));
    }

    #[inline]
    pub fn on_pub_event(&mut self, callback: impl Fn(PublicEvent) -> () + Send + Sync + 'static) {
        self.pub_callbacks.push(Arc::new(callback));
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
            .find(|player| !Rc::ptr_eq(*player, self.active.as_ref().unwrap()))
            .cloned()
    }

    pub fn cutt_deck(
        &mut self,
        player: Rc<RefCell<Player>>,
        mut cards_to_take: usize,
    ) -> Result<(), PlayerError> {
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

    pub fn draw_card_after_trick(
        &mut self,
        player: Rc<RefCell<Player>>,
    ) -> Result<(), PlayerError> {
        if !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Err(PlayerError::CantTakeCardPlayerNotActive);
        }
        if !self.stack.is_empty() {}

        if player.borrow().cards.len() == 5 {
            return Err(PlayerError::CantTakeCardHaveAlreadyFive);
        }

        if self.trump.is_none() {
            return Err(PlayerError::CantTakeCardDeckEmpty);
        }

        if self.closed_talon.is_some() {
            return Err(PlayerError::TalonAlreadyClosed);
        }

        if let Err(PlayerError::CantTakeCardDeckEmpty) = self.draw_card(player.clone()) {
            self.take_trump(player);
        }

        self.swap_to(self.get_non_active_player().unwrap());

        let new = self.active.as_deref().unwrap().borrow();
        if new.cards.len() < 5 && self.trump.is_some() {
            self.notify_priv(new.id.clone(), PrivateEvent::AllowDrawCard);
        } else {
            if new.announcable.len() > 0 {
                self.notify_priv(new.id.clone(), PrivateEvent::AllowAnnounce);
            }
            self.notify_priv(new.id.clone(), PrivateEvent::AllowCloseTalon);
            self.notify_priv(new.id.clone(), PrivateEvent::AllowPlayCard);
        }
        Ok(())
    }

    fn draw_card(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if self.deck.is_empty() {
            return Err(PlayerError::CantTakeCardDeckEmpty);
        }

        let card = self.deck.pop().unwrap();
        player.borrow_mut().cards.push(card.clone());

        self.notify_pub(PublicEvent::DeckCardCount(self.deck.len()));

        self.notify_priv(player.borrow().id.clone(), PrivateEvent::CardAvailabe(card));
        self.notify_pub(PublicEvent::ReceiveCard {
            user_id: player.borrow().id.clone(),
        });
        self.update_announcable_props(player.clone());
        self.run_swap_trump_check(player.clone());
        self.update_playable_cards(player);

        Ok(())
    }

    pub fn close_talon(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if self.active.is_none()
            || !Rc::ptr_eq(&player, self.active.as_ref().unwrap())
            || self.deck.is_empty()
            || !self.stack.is_empty()
        {
            return Err(PlayerError::PlayerNotActive);
        }

        if self.closed_talon.is_some() {
            return Err(PlayerError::TalonAlreadyClosed);
        }

        self.closed_talon = Some(player.clone());
        self.notify_pub(PublicEvent::CloseTalon {
            user_id: player.borrow().id.clone(),
        });

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
            self.notify_priv(player.borrow().id.clone(), PrivateEvent::CardAvailabe(card));
            self.notify_pub(PublicEvent::ReceiveCard {
                user_id: player.borrow().id.clone(),
            });
        }

        self.notify_pub(PublicEvent::DeckCardCount(self.deck.len()));
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
            return Err(PlayerError::NoPlayerActive);
        }
        let mut player_order: Vec<Rc<RefCell<Player>>> = self.players.iter().cloned().collect();

        if !Rc::ptr_eq(self.players.index(0), &self.active.clone().unwrap()) {
            player_order = self.players.iter().cloned().rev().collect();
        }

        let mut callbacks = Vec::new();
        for player in player_order.clone() {
            for _ in 0..3 {
                callbacks.extend(self.do_cards(player.clone()));
            }
        }
        let trump = self.deck.pop().unwrap();
        self.notify_pub(PublicEvent::TrumpChange(Some(trump.clone())));
        let _ = self.trump.insert(trump);

        for player in player_order.clone() {
            for _ in 0..2 {
                callbacks.extend(self.do_cards(player.clone()));
            }
        }

        callbacks.extend(
            player_order
                .into_iter()
                .map(|player| {
                    self.update_playable_cards(player.clone())
                        .into_iter()
                        .chain(self.run_swap_trump_check(player.clone()).into_iter())
                        .chain(self.update_announcable_props(player).into_iter())
                })
                .flatten(),
        );

        let player_id = self.active.as_ref().unwrap().borrow().id.clone();

        let priv_calls = self.priv_callbacks.get(&player_id).unwrap().clone();
        tokio::task::spawn(join_all(callbacks).then(move |_| async move {
            join_all(Self::notify(
                priv_calls.clone(),
                PrivateEvent::AllowPlayCard,
            ))
            .await;
            join_all(Self::notify(priv_calls, PrivateEvent::AllowCloseTalon)).await;
        }));

        Ok(())
    }

    #[inline]
    pub fn set_active_player(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        if self.active.is_some() {
            return Err(PlayerError::CantSetActivePlayer);
        }
        self.make_active(player);
        Ok(())
    }

    pub fn play_card(
        &mut self,
        player: Rc<RefCell<Player>>,
        card: Card,
    ) -> Result<(), PlayerError> {
        if self.active.is_none() || !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Err(PlayerError::PlayerNotActive);
        }

        if !player.borrow().playable_cards.contains(&card) {
            return Err(PlayerError::CantPlayCard(card));
        }

        player.borrow_mut().cards.retain(|c| *c != card);
        player.borrow_mut().playable_cards.retain(|c| *c != card);

        self.stack.push(card.clone());
        let player_id = player.borrow().id.clone();
        self.notify_priv(
            player_id.clone(),
            PrivateEvent::CardUnavailabe(card.clone()),
        );

        self.notify_priv(
            player_id.clone(),
            PrivateEvent::CardNotPlayable(card.clone()),
        );

        self.notify_pub(PublicEvent::PlayCard {
            user_id: player_id,
            card,
        });

        if self.stack.len() == 2 {
            self.handle_trick()?;
        } else {
            self.swap_to(self.get_non_active_player().unwrap());
            self.notify_priv(
                self.active.as_ref().unwrap().borrow().id.clone(),
                PrivateEvent::AllowPlayCard,
            );
        }
        Ok(())
    }

    pub fn swap_trump(
        &mut self,
        player: Rc<RefCell<Player>>,
        card: Card,
    ) -> Result<(), PlayerError> {
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

                self.notify_priv(borrow.id.clone(), PrivateEvent::CardAvailabe(trump));
                self.notify_priv(borrow.id.clone(), PrivateEvent::CardUnavailabe(other));
            }

            self.notify_pub(PublicEvent::TrumpChange(Some(
                self.trump.as_ref().unwrap().clone(),
            )));
            self.update_playable_cards(player);
            return Ok(());
        }
        Err(PlayerError::CantSwapTrump)
    }

    pub fn announce_40(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        let cards_to_announce = self.can_announce_40(player.clone());
        if cards_to_announce.is_none() {
            return Err(PlayerError::CantPlay40);
        }

        let announcement = models::Announcement {
            cards: cards_to_announce.unwrap(),
            announce_type: models::AnnounceType::Forty,
        };

        player
            .borrow_mut()
            .announcements
            .push(announcement.clone().into());

        self.notify_pub(PublicEvent::Announce {
            user_id: player.borrow().id.clone(),
            announcement: announcement.clone(),
        });
        self.run_after_move_checks();
        self.notify_changes_playable_cards(player.clone(), &announcement.cards);
        self.update_announcable_props(player.clone());
        self.update_finish_round(player)
    }

    pub fn announce_20(
        &mut self,
        player: Rc<RefCell<Player>>,
        cards: [Card; 2],
    ) -> Result<(), PlayerError> {
        let announce = self.can_announce_20(player.clone());
        if announce.is_empty() || !contains_card_comb(&announce, cards.clone()) {
            return Err(PlayerError::CantPlay20);
        }

        let announcement = models::Announcement {
            cards,
            announce_type: models::AnnounceType::Twenty,
        };

        player
            .borrow_mut()
            .announcements
            .push(announcement.clone().into());

        self.notify_pub(PublicEvent::Announce {
            user_id: player.borrow().id.clone(),
            announcement: announcement.clone(),
        });
        self.run_after_move_checks();
        self.notify_changes_playable_cards(player.clone(), &announcement.cards);
        self.update_announcable_props(player.clone());
        self.update_finish_round(player)
    }

    #[inline]
    // TODO: This function returns a fucking value because of some stupid fucking borrowing rules your mum invented. Change this to a fucking stupid refernce as soon as possible
    fn can_announce_20(&self, player: Rc<RefCell<Player>>) -> Vec<[Card; 2]> {
        if self.active.is_none() || !Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return Vec::new();
        }
        let player = self.active.as_deref().unwrap();

        (0..4)
            .filter_map(move |suit: u8| {
                let borrow = player.borrow();
                let mut cards_iter = borrow.cards.iter().filter(|card| {
                    card.suit == suit.into()
                        && (card.value == models::CardVal::Queen
                            || card.value == models::CardVal::King)
                });
                if let (Some(card1), Some(card2)) = (cards_iter.next(), cards_iter.next()) {
                    Some([card1.clone(), card2.clone()])
                } else {
                    None
                }
            })
            .collect()
    }

    fn can_announce_40(&self, player: Rc<RefCell<Player>>) -> Option<[Card; 2]> {
        let cards_to_announce = self.can_announce_20(player.clone());
        if cards_to_announce.is_empty() {
            return None;
        }

        for pretender in cards_to_announce {
            // TODO: As this is needed more often, move this into it's own function
            let trump = match self.trump {
                Some(ref trump) => trump,
                None => &self.taken_trump.as_ref().unwrap().1,
            };

            if pretender.first().unwrap().suit == trump.suit {
                return Some(pretender);
            }
        }
        None
    }

    fn populate_deck() -> [Card; 20] {
        let mut deck = (0..4)
            .map(|suit: u8| {
                (10..12).chain(2..5).map(move |value: u8| Card {
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

    // TODO: Change the return type to one simple JoinHandle
    fn notify_pub(&self, event: PublicEvent) -> Vec<tokio::task::JoinHandle<()>> {
        Self::notify(self.pub_callbacks.clone(), event)
    }

    fn notify<T: Clone + Send + Sync + 'static>(
        callbacks: Vec<Arc<dyn Fn(T) -> () + Send + Sync + 'static>>,
        event: T,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();
        for callback in callbacks.iter().cloned() {
            let clone = event.clone();
            handles.push(tokio::task::spawn(async move { callback(clone) }));
        }
        handles
    }

    fn notify_priv(
        &self,
        user_id: String,
        event: PrivateEvent,
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

    fn update_announcable_props(
        &self,
        player: Rc<RefCell<Player>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let id = player.borrow().id.clone();

        let announcable_cards = self.can_announce_20(player.clone());
        let mut announcements = announcable_cards
            .iter()
            .map(|card| Announcement {
                cards: card.clone(),
                announce_type: models::AnnounceType::Twenty,
            })
            .collect::<Vec<_>>();

        if let Some(announcement_cards) = self.can_announce_40(player.clone()) {
            let forty_announcement = Announcement {
                cards: announcement_cards,
                announce_type: models::AnnounceType::Forty,
            };

            announcements.push(forty_announcement.clone());
        }

        let player_announcements = player.borrow().announcable.clone();

        let mut callbacks = Vec::new();

        for announcement in player_announcements.iter() {
            if !has_announcable(&announcements, announcement) {
                player.borrow_mut().announcable.retain(|x| {
                    x.announce_type != announcement.announce_type
                        && x.cards.first().unwrap().suit != announcement.cards.first().unwrap().suit
                });

                callbacks.extend(self.notify_priv(
                    id.clone(),
                    PrivateEvent::CannotAnnounce(announcement.clone()),
                ));
            }
        }

        for announcement in announcements {
            if player
                .borrow()
                .announcements
                .iter()
                .any(|x| x.cards.first().unwrap().suit == announcement.cards.first().unwrap().suit)
            {
                player.borrow_mut().announcable.retain(|x| {
                    x.cards.first().unwrap().suit != announcement.cards.first().unwrap().suit
                });
                continue;
            }

            if !player.borrow().has_announcable(&announcement) {
                player.borrow_mut().announcable.push(announcement.clone());
                callbacks
                    .extend(self.notify_priv(id.clone(), PrivateEvent::CanAnnounce(announcement)));
            }
        }
        callbacks
    }

    fn run_swap_trump_check(
        &self,
        player: Rc<RefCell<Player>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut callbacks = Vec::new();
        let can_swap = player.borrow().possible_trump_swap.is_some();
        let id = player.borrow().id.clone();
        if let Some(swap) = self.can_swap_trump(player.clone()) {
            if can_swap {
                return callbacks;
            }
            callbacks.extend(self.notify_priv(
                id,
                PrivateEvent::TrumpChangePossible(player.borrow().cards[swap].clone()),
            ));
            return callbacks;
        }
        if can_swap {
            callbacks.extend(self.notify_priv(
                id,
                PrivateEvent::TrumpChangeImpossible(
                    player.borrow_mut().possible_trump_swap.take().unwrap(),
                ),
            ));
        }
        callbacks
    }

    fn run_after_move_checks(&mut self) {}

    fn swap_to(&mut self, player: Rc<RefCell<Player>>) {
        self.update_playable_cards(player.clone());
        if self.active.is_none() || Rc::ptr_eq(&player, self.active.as_ref().unwrap()) {
            return;
        }
        let user_id = self.active.as_deref().unwrap().borrow().id.clone();
        self.notify_pub(PublicEvent::Inactive {
            user_id: user_id.clone(),
        });

        self.make_active(player);
    }

    fn make_active(&mut self, player: Rc<RefCell<Player>>) -> Vec<tokio::task::JoinHandle<()>> {
        let user_id = self.active.insert(player).borrow().id.clone();

        self.notify_pub(PublicEvent::Active { user_id })
    }

    // First player in the array is the player who first played the card (first card on the stack)
    fn get_winner<'a>(
        &self,
        players: [&'a (Rc<RefCell<Player>>, Card); 2],
    ) -> &'a (Rc<RefCell<Player>>, Card) {
        let enemy = players.first().unwrap();
        let active = players.last().unwrap();

        let enemy_suit = enemy.1.suit.clone();
        let active_suit = active.1.suit.clone();
        let enemy_is_trump = {
            if self.trump.is_some() {
                enemy_suit == self.trump.as_ref().unwrap().suit
            } else {
                enemy_suit == self.taken_trump.as_ref().unwrap().1.suit
            }
        };

        let active_is_trump = {
            if self.trump.is_some() {
                active_suit == self.trump.as_ref().unwrap().suit
            } else {
                active_suit == self.taken_trump.as_ref().unwrap().1.suit
            }
        };

        if !active_is_trump && enemy_is_trump {
            enemy
        } else if active_is_trump && !enemy_is_trump {
            active
        } else if self.stack.last().unwrap().suit != self.stack.first().unwrap().suit {
            enemy
        } else if self.stack.last().unwrap().value.clone() as u8
            > self.stack.first().unwrap().value.clone() as u8
        {
            active
        } else {
            enemy
        }
    }

    fn update_finish_round(&mut self, last_trick: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        let comparison_result = self.update_points().unwrap();

        let mut winner = comparison_result.winner;
        let mut loser = comparison_result.loser;

        if winner.points < 66 {
            if self
                .players
                .iter()
                .all(|player| player.borrow().cards.is_empty())
            {
                winner.points += loser.points;
                loser.points = 0;

                if let Some(ref player) = self.closed_talon {
                    winner.points += 10;

                    if Rc::ptr_eq(&player, &winner.player) {
                        std::mem::swap(&mut winner.player, &mut loser.player);
                    }
                } else if !Rc::ptr_eq(&last_trick, &winner.player) {
                    std::mem::swap(&mut winner.player, &mut loser.player);
                }
            } else {
                return Ok(());
            }
        }

        let points;
        if loser.points == 0 {
            points = 3;
        } else if loser.points >= 33 {
            points = 2;
        } else {
            points = 1;
        }

        winner.player.borrow_mut().points += points;

        let mut ranked = HashMap::new();
        {
            let winner = winner.player.borrow();
            let loser = loser.player.borrow();
            ranked.insert(winner.id.clone(), winner.points);
            ranked.insert(loser.id.clone(), loser.points);
        }

        self.notify_pub(PublicEvent::Result {
            winner: winner.player.borrow().id.clone(),
            points,
            ranked,
        });

        winner.player.borrow_mut().reset();
        loser.player.borrow_mut().reset();

        if self
            .players
            .iter()
            .find(|player| player.borrow().points >= 7)
            .is_none()
        {
            return Ok(());
        }

        let mut res = HashMap::new();
        res.insert(
            winner.player.borrow().id.clone(),
            winner.player.borrow().points,
        );
        res.insert(
            loser.player.borrow().id.clone(),
            loser.player.borrow().points,
        );

        self.notify_pub(PublicEvent::FinalResult {
            ranked: res,
            winner: winner.player.borrow().id.clone(),
        });
        Ok(())
    }

    fn update_points(&mut self) -> Result<CardComparisonResult, PlayerError> {
        let points = self
            .players
            .iter()
            .map(|player| {
                let player = player.borrow();
                player.tricks.iter().flatten().fold(
                    player
                        .announcements
                        .iter()
                        .map(|a| {
                            if !player.tricks.is_empty() {
                                a.announce_type.clone() as u8
                            } else {
                                0 as u8
                            }
                        })
                        .sum(),
                    |acc, card| acc + card.value.clone() as u8,
                )
            })
            .zip(self.players.iter());

        let (max_points, winner) = points.clone().max_by_key(|(points, _)| *points).unwrap();
        let (min_points, loser) = points.min_by_key(|(points, _)| *points).unwrap();

        self.notify_pub(PublicEvent::Score {
            user_id: winner.borrow().id.clone(),
            points: max_points,
        });

        self.notify_pub(PublicEvent::Score {
            user_id: loser.borrow().id.clone(),
            points: min_points,
        });

        Ok(CardComparisonResult {
            winner: PlayerPoint {
                player: winner.clone(),
                points: max_points,
            },
            loser: PlayerPoint {
                player: loser.clone(),
                points: min_points,
            },
        })
    }

    fn handle_trick(&mut self) -> Result<(), PlayerError> {
        let won = self
            .get_winner([
                &(
                    self.get_non_active_player().unwrap(),
                    self.stack.first().unwrap().clone(),
                ),
                &(
                    self.get_active_player().unwrap(),
                    self.stack.last().unwrap().clone(),
                ),
            ])
            .0
            .clone();

        let won_id = won.borrow().id.clone();
        self.notify_pub(PublicEvent::Trick {
            user_id: won_id.clone(),
            cards: self.stack.clone().try_into().unwrap(),
        });

        won.as_ref()
            .borrow_mut()
            .tricks
            .push([self.stack.pop().unwrap(), self.stack.pop().unwrap()]);

        self.swap_to(won.clone());

        if !self.deck.is_empty() && self.closed_talon.is_none() {
            self.notify_priv(won_id.clone(), PrivateEvent::AllowDrawCard);
        } else {
            if won.borrow().announcable.len() > 0 {
                self.notify_priv(won.borrow().id.clone(), PrivateEvent::AllowAnnounce);
            }
            self.notify_priv(won_id, PrivateEvent::AllowPlayCard);
        }

        self.update_finish_round(won)
    }

    fn do_cards(&mut self, player: Rc<RefCell<Player>>) -> Vec<tokio::task::JoinHandle<()>> {
        let card = self.deck.pop().unwrap();
        let mut callbacks = self.notify_priv(
            player.as_ref().borrow().id.clone(),
            PrivateEvent::CardAvailabe(card.clone()),
        );
        callbacks.extend(self.notify_pub(PublicEvent::ReceiveCard {
            user_id: player.as_ref().borrow().id.clone(),
        }));
        player.as_ref().borrow_mut().cards.push(card);
        callbacks
    }

    fn next_round(&mut self, winner: Rc<RefCell<Player>>) {
        self.active = None;
        self.trump = None;
        self.stack.clear();
        self.closed_talon = None;
        self.taken_trump = None;
        self.players.iter().for_each(|player| {
            player.borrow_mut().reset();
        });

        self.recreate_deck();
        self.distribute_cards().unwrap();

        self.make_active(winner.clone());
    }

    fn can_swap_trump(&self, player: Rc<RefCell<Player>>) -> Option<usize> {
        if self.active.is_none()
            || self.trump.is_none()
            || !Rc::ptr_eq(&player, self.active.as_ref().unwrap())
            || !self.stack.is_empty()
            || self.closed_talon.is_some()
        {
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

    fn notify_changes_playable_cards(
        &self,
        player: Rc<RefCell<Player>>,
        playable: &[Card],
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut callbacks: Vec<_> = playable
            .iter()
            .filter_map(|card| {
                if player
                    .borrow()
                    .playable_cards
                    .iter()
                    .any(|x| x.to_owned() == card.to_owned())
                {
                    return None;
                }
                Some(self.notify_priv(
                    player.borrow().id.clone(),
                    PrivateEvent::CardPlayable(card.clone()),
                ))
            })
            .into_iter()
            .collect();

        callbacks.extend(player.borrow().playable_cards.iter().filter_map(|card| {
            if !playable.iter().any(|x| x.to_owned() == card.to_owned()) {
                return Some(self.notify_priv(
                    player.borrow().id.clone(),
                    PrivateEvent::CardNotPlayable(card.clone()),
                ));
            }
            None
        }));

        player.borrow_mut().playable_cards = playable.to_vec();
        callbacks.into_iter().flatten().collect()
    }

    fn update_playable_cards(
        &self,
        player: Rc<RefCell<Player>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut playable = {
            if !self.stack.is_empty() && (self.taken_trump.is_some() || self.closed_talon.is_some())
            {
                let trump = match self.trump {
                    Some(ref trump) => trump,
                    None => &self.taken_trump.as_ref().unwrap().1,
                };

                let player = player.borrow();

                // Force color
                let forcing_color: Vec<_> = player
                    .cards
                    .iter()
                    .filter(|card| card.suit == self.stack.first().unwrap().suit.clone())
                    .cloned()
                    .collect();

                if forcing_color.is_empty() {
                    player
                        .cards
                        .iter()
                        .filter(|card| card.suit == trump.suit)
                        .cloned()
                        .collect()
                } else {
                    forcing_color
                }
            } else {
                player.borrow().cards.clone()
            }
        };

        if playable.is_empty() {
            playable = player.borrow().cards.clone();
        }

        if self.closed_talon.is_some() && !self.stack.is_empty() {
            // Force trick
            let gonna_win: Vec<_> = playable
                .clone()
                .into_iter()
                .filter(|card| {
                    self.get_winner([
                        &(
                            self.get_non_active_player().unwrap().clone(),
                            self.stack.first().unwrap().clone(),
                        ),
                        &(self.get_active_player().unwrap(), card.clone()),
                    ])
                    .0
                    .borrow()
                    .id == player.borrow().id
                })
                .collect();

            if !gonna_win.is_empty() {
                playable = gonna_win;
            }
        }

        self.notify_changes_playable_cards(player, &playable)
    }

    fn take_trump(&mut self, player: Rc<RefCell<Player>>) {
        let taken_trump = self
            .taken_trump
            .insert((player.clone(), self.trump.take().unwrap()))
            .clone();

        player.borrow_mut().cards.push(taken_trump.1.clone());

        self.notify_pub(PublicEvent::TrumpChange(None));
        // TODO: Everything after this line should be consolidated into a function, as it is also needed in self.draw_card
        self.notify_pub(PublicEvent::DeckCardCount(self.deck.len()));
        self.notify_priv(
            player.borrow().id.clone(),
            PrivateEvent::CardAvailabe(taken_trump.1),
        );
        self.notify_pub(PublicEvent::ReceiveCard {
            user_id: player.borrow().id.clone(),
        });
        self.update_announcable_props(player.clone());
        self.run_swap_trump_check(player.clone());
        self.update_playable_cards(player);
    }
}
