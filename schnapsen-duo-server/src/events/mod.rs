use event_logger::{EventLike};
use serde::Serialize;

use crate::emitter::EventIdentifier;


pub mod event_logger;


#[derive(Serialize, Hash, Debug, PartialEq, Eq, Clone)]
pub enum EventType<Prv, Pub> {
    Private(Prv),
    Public(Pub),
}

impl<Prv, Pub> EventLike for EventType<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
}

impl<Prv, Pub> EventIdentifier for EventType<Prv, Pub>
where
    Prv: EventIdentifier,
    Pub: EventIdentifier,
{
    fn event_name(&self) -> &'static str {
        match self {
            EventType::Private(event) => event.event_name(),
            EventType::Public(event) => event.event_name(),
        }
    }
}

impl EventLike for schnapsen_rs::PrivateEvent {}
impl EventLike for schnapsen_rs::PublicEvent {}

#[derive(Serialize, Debug, Clone)]
pub struct TimedEvent<T>
where
    T: EventIdentifier + Clone + Serialize,
{
    #[serde(flatten)]
    pub event: T,
    pub timestamp: u64,
}

impl<T> EventIdentifier for TimedEvent<T>
where
    T: EventIdentifier + Clone + Serialize,
{
    fn event_name(&self) -> &'static str {
        self.event.event_name()
    }
}

impl<T> From<T> for TimedEvent<T>
where
    T: EventIdentifier + Clone + Serialize,
{
    fn from(event: T) -> Self {
        Self {
            event,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }
}

impl EventIdentifier for schnapsen_rs::PrivateEvent {
    fn event_name(&self) -> &'static str {
        match self {
            schnapsen_rs::PrivateEvent::CanAnnounce(_) => "can_announce",
            schnapsen_rs::PrivateEvent::CardAvailabe(_) => "card_available",
            schnapsen_rs::PrivateEvent::CardUnavailabe(_) => "card_unavailable",
            schnapsen_rs::PrivateEvent::TrumpChangePossible(_) => "trump_change_possible",
            schnapsen_rs::PrivateEvent::CardPlayable(_) => "card_playable",
            schnapsen_rs::PrivateEvent::CardNotPlayable(_) => "card_not_playable",
            schnapsen_rs::PrivateEvent::AllowDrawCard => "allow_draw_card",
            schnapsen_rs::PrivateEvent::AllowPlayCard => "allow_play_card",
            schnapsen_rs::PrivateEvent::AllowAnnounce => "allow_announce",
            schnapsen_rs::PrivateEvent::AllowCloseTalon => "allow_close_talon",
            schnapsen_rs::PrivateEvent::CannotAnnounce(_) => "cannot_announce",
            schnapsen_rs::PrivateEvent::TrumpChangeImpossible(_) => "trump_change_impossible",
            schnapsen_rs::PrivateEvent::AllowSwapTrump => "allow_swap_trump",
        }
    }
}

impl EventIdentifier for schnapsen_rs::PublicEvent {
    fn event_name(&self) -> &'static str {
        match self {
            schnapsen_rs::PublicEvent::Active { .. } => "active",
            schnapsen_rs::PublicEvent::Announce { .. } => "announce",
            schnapsen_rs::PublicEvent::CloseTalon { .. } => "close_talon",
            schnapsen_rs::PublicEvent::DeckCardCount { .. } => "deck_card_count",
            schnapsen_rs::PublicEvent::FinalResult { .. } => "final_result",
            schnapsen_rs::PublicEvent::Inactive { .. } => "inactive",
            schnapsen_rs::PublicEvent::PlayCard { .. } => "play_card",
            schnapsen_rs::PublicEvent::ReceiveCard { .. } => "receive_card",
            schnapsen_rs::PublicEvent::Trick { .. } => "trick",
            schnapsen_rs::PublicEvent::TrumpChange { .. } => "trump_change",
            schnapsen_rs::PublicEvent::Result { .. } => "result",
            schnapsen_rs::PublicEvent::FinishedDistribution { .. } => "finished_distribution",
            schnapsen_rs::PublicEvent::Score { .. } => "score",
        }
    }
}

pub type SchnapsenDuoEventType =
    EventType<schnapsen_rs::PrivateEvent,schnapsen_rs::PublicEvent>;
