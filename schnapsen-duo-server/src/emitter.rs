use schnapsen_rs::{PrivateEvent, PublicEvent};
use socketioxide::{extract::SocketRef, operators::BroadcastOperators};
use tracing::debug;

pub fn to_private_event_emitter<'a>(
    event: &'a PrivateEvent,
) -> impl Fn(SocketRef) -> Result<(), socketioxide::SendError<PrivateEvent>> + 'a {
    let event_name: &'static str = match &event {
        PrivateEvent::CanAnnounce(_) => "can_announce",
        PrivateEvent::CardAvailabe(_) => "card_available",
        PrivateEvent::CardUnavailabe(_) => "card_unavailable",
        PrivateEvent::TrumpChangePossible(_) => "trump_change_possible",
        PrivateEvent::CardPlayable(_) => "card_playable",
        PrivateEvent::CardNotPlayable(_) => "card_not_playable",
        PrivateEvent::AllowDrawCard => "allow_draw_card",
        PrivateEvent::AllowPlayCard => "allow_play_card",
        PrivateEvent::AllowAnnounce => "allow_announce",
        PrivateEvent::AllowCloseTalon => "allow_close_talon",
        PrivateEvent::CannotAnnounce(_) => "cannot_announce",
        PrivateEvent::TrumpChangeImpossible(_) => "trump_change_impossible", 
    };
    debug!("Emitting private event: {}", event_name);
    move |socket: SocketRef| socket.emit(event_name, event.clone())
}

pub fn to_public_event_emitter<'a>(
    event: &'a PublicEvent,
) -> impl Fn(BroadcastOperators) -> Result<(), socketioxide::BroadcastError> + 'a {
    let event_name: &'static str = match event {
        PublicEvent::Active { .. } => "active",
        PublicEvent::Announce { .. } => "announce",
        PublicEvent::CloseTalon { .. } => "close_talon",
        PublicEvent::DeckCardCount { .. } => "deck_card_count",
        PublicEvent::FinalResult { .. } => "final_result",
        PublicEvent::Inactive { .. } => "inactive",
        PublicEvent::PlayCard { .. } => "play_card",
        PublicEvent::ReceiveCard { .. } => "receive_card",
        PublicEvent::Trick { .. } => "trick",
        PublicEvent::TrumpChange { .. } => "trump_change",
        PublicEvent::Result { .. } => "result",
        PublicEvent::FinishedDistribution { .. } => "finished_distribution",
        PublicEvent::Score { .. } => "score",
    };

    debug!("Emitting public event: {}", event_name);
    move |socket: BroadcastOperators| socket.emit(event_name, event.clone())
}
