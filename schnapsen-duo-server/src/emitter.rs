use schnapsen_rs::{PrivateEvent, PublicEvent};
use serde::Serialize;
use socketioxide::{extract::SocketRef, operators::BroadcastOperators};
use tracing::debug;

pub trait EventIdentifier {
    fn event_name(&self) -> &'static str;
}

pub fn to_private_event_emitter<'a, T>(
    event: &'a T,
) -> impl Fn(SocketRef) -> Result<(), socketioxide::SendError<T>> + 'a
where
    T: EventIdentifier + Clone + Serialize,
{
    let event_name: &'static str = event.event_name();
    debug!("Emitting private event: {}", event_name);
    move |socket: SocketRef| socket.emit(event_name, event.clone())
}

pub fn to_public_event_emitter<'a, T>(
    event: &'a T,
) -> impl Fn(BroadcastOperators) -> Result<(), socketioxide::BroadcastError> + 'a
where
    T: EventIdentifier + Clone + Serialize,
{
    let event_name: &'static str = event.event_name();
    debug!("Emitting public event: {}", event_name);
    move |socket: BroadcastOperators| socket.emit(event_name, event.clone())
}
