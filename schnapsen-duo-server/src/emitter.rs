use schnapsen_rs::{PrivateEvent, PublicEvent};
use socketioxide::extract::SocketRef;

pub fn to_private_event_emitter<'a>(event: &'a PrivateEvent, socket: SocketRef) -> impl Fn() -> Result<(), socketioxide::SendError<PrivateEvent>> + 'a {
    let event_name: &'static str = match &event {
        PrivateEvent::Active => "active",
        PrivateEvent::CanAnnounce(_) => "can_announce",
        PrivateEvent::CardAvailabe(_) => "card_available",
        PrivateEvent::CardUnavailabe(_) => "card_unavailable",
        PrivateEvent::Inactive => "inactive",
        PrivateEvent::Result(_) => "result",
        PrivateEvent::Trick(_) => "trick",
        PrivateEvent::TrumpChange(_) => "trump_change",
    };

    move || socket.emit(event_name, event.clone())
}

pub fn to_public_event_emitter<'a>(event: &'a PublicEvent, socket: SocketRef) -> impl Fn() -> Result<(), socketioxide::SendError<PublicEvent>> + 'a
{
    let event_name: &'static str = match event {
        PublicEvent::Active(_) => "active",
        PublicEvent::Announce(_) => "announce",
        PublicEvent::CloseTalon => "close_talon",
        PublicEvent::DeckCardCount(_) => "deck_card_count",
        PublicEvent::FinalResult(_) => "final_result",
        PublicEvent::Inactive(_) => "inactive",
        PublicEvent::PlayCard(_) => "play_card",
        PublicEvent::ReceiveCard(_) => "receive_card",
        PublicEvent::Trick(_, _) => "trick",
        PublicEvent::TrumpChange(_) => "trump_change",
        PublicEvent::Result(_, _) => "result",
    };

    move || socket.emit(event_name, event.clone())
}
