use core::fmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Index;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::join_all;
use futures::FutureExt;
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
    CantTakeCardPlayerNotActive,
    CantTakeCardHaveAlreadyFive,
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
            },
            PlayerError::CantTakeCardPlayerNotActive => {
                "Can't take card because player is not active"
            },
            PlayerError::CantTakeCardHaveAlreadyFive => {
                "Can't take card because player already has five cards"
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
}

// TODO: Alle user_ids are currently serialized as the write-tokens. This has to be changed. SECURITY RISK
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PublicEvent {
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
    TrumpChange(Card),
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
        if !self.stack.is_empty() {
            return Err(PlayerError::CantTakeCardRoundNotFinished);
        }

        if player.borrow().cards.len() == 5 {
            return Err(PlayerError::CantTakeCardHaveAlreadyFive);
        }
        if let Err(PlayerError::CantTakeCardDeckEmpty) = self.draw_card(player.clone()) {
            self.take_trump(player);
        }

        self.swap_to(self.get_non_active_player().unwrap());

        let new = self.active.as_deref().unwrap().borrow();
        if new.cards.len() < 5 {
            self.notify_priv(new.id.clone(), PrivateEvent::AllowDrawCard);
        } else {
            if new.announcable.is_some() {
                self.notify_priv(new.id.clone(), PrivateEvent::AllowAnnounce);
            }
            self.notify_priv(new.id.clone(), PrivateEvent::AllowPlayCard);
        }
        Ok(())
    }

    pub fn draw_card(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
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
            return Err(PlayerError::CantDistributeCardsNoPlayerActive);
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
        self.notify_pub(PublicEvent::TrumpChange(trump.clone()));
        let _ = self.trump.insert(trump);

        for player in player_order.clone() {
            for _ in 0..2 {
                callbacks.extend(self.do_cards(player.clone()));
            }
        }

        callbacks.extend(
            player_order
                .into_iter()
                .map(|player| self.update_playable_cards(player))
                .flatten(),
        );

        let player_id = self.active.as_ref().unwrap().borrow().id.clone();

        let priv_calls = self.priv_callbacks.get(&player_id).unwrap().clone();
        tokio::task::spawn(join_all(callbacks).then(move |_| async move {
            join_all(Self::notify(priv_calls, PrivateEvent::AllowPlayCard)).await;
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

            self.notify_pub(PublicEvent::TrumpChange(
                self.trump.as_ref().unwrap().clone(),
            ));
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

        player.borrow_mut().announcement = Some(announcement.clone());
        self.notify_pub(PublicEvent::Announce {
            user_id: player.borrow().id.clone(),
            announcement: announcement.clone(),
        });
        self.run_after_move_checks();
        Ok(())
    }

    pub fn announce_20(&mut self, player: Rc<RefCell<Player>>) -> Result<(), PlayerError> {
        let announce = self.can_announce_20(player.clone());
        if announce.is_none() {
            return Err(PlayerError::CantPlay20);
        }
        let announce = announce.unwrap();

        let announcement = models::Announcement {
            cards: announce,
            announce_type: models::AnnounceType::Forty,
        };

        player.borrow_mut().announcement = Some(announcement.clone());
        self.notify_pub(PublicEvent::Announce {
            user_id: player.borrow().id.clone(),
            announcement: announcement.clone(),
        });
        self.run_after_move_checks();
        Ok(())
    }

    #[inline]
    // TODO: This function returns a fucking value because of some stupid fucking borrowing rules your mum invented. Change this to a fucking stupid refernce as soon as possible
    fn can_announce_20(&self, player: Rc<RefCell<Player>>) -> Option<[Card; 2]> {
        if self.active.is_none()
            || !Rc::ptr_eq(&player, self.active.as_ref().unwrap())
            || player.borrow().announcement.is_some()
        {
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

    fn update_announcable_props(&self, player: Rc<RefCell<Player>>) {
        let id = player.borrow().id.clone();
        let has_announceable = player.borrow().announcable.is_some();
        let mut announcement;
        if let Some(announcement_cards) = self.can_announce_20(player.clone()) {
            announcement = Announcement {
                cards: announcement_cards,
                announce_type: models::AnnounceType::Twenty,
            };
        } else {
            if !has_announceable {
                return;
            }
            self.notify_priv(
                id,
                PrivateEvent::CannotAnnounce(player.borrow_mut().announcable.take().unwrap()),
            );
            return;
        }

        if has_announceable {
            return;
        }

        if let Some(announcement_cards) = self.can_announce_40(player.clone()) {
            announcement = Announcement {
                cards: announcement_cards,
                announce_type: models::AnnounceType::Forty,
            };
        }

        player.borrow_mut().announcable = Some(announcement.clone());
        self.notify_priv(id, PrivateEvent::CanAnnounce(announcement));
    }

    fn run_swap_trump_check(&self, player: Rc<RefCell<Player>>) {
        let can_swap = player.borrow().possible_trump_swap.is_some();
        let id = player.borrow().id.clone();
        if let Some(swap) = self.can_swap_trump(player.clone()) {
            if can_swap {
                return;
            }
            self.notify_priv(
                id,
                PrivateEvent::TrumpChangePossible(player.borrow().cards[swap].clone()),
            );
            return;
        }
        if can_swap {
            self.notify_priv(
                id,
                PrivateEvent::TrumpChangeImpossible(
                    player.borrow_mut().possible_trump_swap.take().unwrap(),
                ),
            );
        }
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

    fn handle_trick(&mut self) -> Result<(), PlayerError> {
        let won = {
            let enemy = self.get_non_active_player().unwrap();

            let enemy_suit = self.stack.first().unwrap().suit.clone();
            let active_suit = self.stack.last().unwrap().suit.clone();
            let enemy_is_trump = {
                if self.trump.is_some() {
                    enemy_suit == self.trump.as_ref().unwrap().suit
                } else {
                    active_suit != self.taken_trump.as_ref().unwrap().1.suit
                }
            };

            let active_is_trump = {
                if self.trump.is_some() {
                    active_suit == self.trump.as_ref().unwrap().suit
                } else {
                    active_suit == enemy_suit
                        || active_suit == self.taken_trump.as_ref().unwrap().1.suit
                }
            };

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

        let won_id = won.borrow().id.clone();
        self.swap_to(won.clone());
        self.notify_pub(PublicEvent::Trick {
            user_id: won_id.clone(),
            cards: self.stack.clone().try_into().unwrap(),
        });

        won.as_ref()
            .borrow_mut()
            .tricks
            .push([self.stack.pop().unwrap(), self.stack.pop().unwrap()]);

        if !self.deck.is_empty() {
            self.notify_priv(won_id, PrivateEvent::AllowDrawCard);
        } else {
            self.notify_priv(won_id, PrivateEvent::AllowPlayCard);
        }

        let points = self
            .players
            .iter()
            .map(|player| {
                let player = player.borrow();
                player.tricks.iter().flatten().fold(
                    player
                        .announcement
                        .as_ref()
                        .map(|a| {
                            if player.tricks.len() > 0 {
                                a.announce_type.clone() as u8
                            } else {
                                0 as u8
                            }
                        })
                        .unwrap_or(0),
                    |acc, card| acc + card.value.clone() as u8,
                )
            })
            .zip(self.players.iter());

        let (max_points, winner) = points.clone().max_by_key(|(points, _)| *points).unwrap();
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

        winner.borrow_mut().points += points;

        let mut ranked = HashMap::new();
        {
            let winner = winner.borrow();
            let loser = loser.borrow();
            ranked.insert(winner.id.clone(), max_points);
            ranked.insert(loser.id.clone(), min_points);
        }

        self.notify_pub(PublicEvent::Result {
            winner: winner.borrow().id.clone(),
            points,
            ranked,
        });

        winner.as_ref().borrow_mut().reset();
        loser.as_ref().borrow_mut().reset();

        if self
            .players
            .iter()
            .find(|player| player.borrow().points >= 7)
            .is_none()
        {
            return Ok(());
        }

        let mut res = HashMap::new();
        res.insert(winner.borrow().id.clone(), winner.borrow().points);
        res.insert(loser.borrow().id.clone(), loser.borrow().points);

        self.notify_pub(PublicEvent::FinalResult {
            ranked: res,
            winner: winner.borrow().id.clone(),
        });
        Ok(())
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

    fn can_swap_trump(&self, player: Rc<RefCell<Player>>) -> Option<usize> {
        if self.active.is_none()
            || self.trump.is_none()
            || !Rc::ptr_eq(&player, self.active.as_ref().unwrap())
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

    fn update_playable_cards(
        &self,
        player: Rc<RefCell<Player>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut playable = 'inner: {
            let allowed_suit;

            if self.trump.is_some() {
                allowed_suit = self.trump.as_ref().unwrap().suit.clone();
            } else if !self.stack.is_empty() {
                allowed_suit = self.stack.first().unwrap().suit.clone();
            } else {
                break 'inner player.borrow().cards.clone();
            }

            player
                .borrow()
                .cards
                .iter()
                .filter(|card| card.suit == allowed_suit)
                .cloned()
                .collect()
        };

        if playable.is_empty() {
            playable = player.borrow().cards.clone();
        }

        let mut callbacks: Vec<_> = playable
            .iter()
            .filter_map(|card| {
                if player.borrow().playable_cards.contains(card) {
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
            if !playable.contains(card) {
                return Some(self.notify_priv(
                    player.borrow().id.clone(),
                    PrivateEvent::CardNotPlayable(card.clone()),
                ));
            }
            None
        }));

        player.borrow_mut().playable_cards = playable;
        callbacks.into_iter().flatten().collect()
    }

    fn take_trump(&mut self, player: Rc<RefCell<Player>>) {
        let taken_trump = self
            .taken_trump
            .insert((player.clone(), self.trump.take().unwrap()));
        player.borrow_mut().cards.push(taken_trump.1.clone());
    }
}
