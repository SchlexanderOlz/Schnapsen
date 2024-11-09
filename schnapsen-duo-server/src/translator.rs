use std::{
    default,
    sync::{Arc, Mutex},
};

use num_enum::FromPrimitive;
use schnapsen_rs::models::Card;
use socketioxide::{
    adapter::LocalAdapter,
    extract::{Data, SocketRef},
    handler::MessageHandler,
};
use tracing::debug;

#[derive(Clone, Debug)]
pub enum SchnapsenDuoActions {
    PlayCard(Card),
    Quit,
    SwapTrump(Card),
    CloseTalon,
    Announce20([Card; 2]),
    Announce40,
    DrawCard,
    CuttDeck(usize),
    TakeCards(usize),
}

impl SchnapsenDuoActions {
    pub const fn into_int(self) -> u8 {
        match self {
            SchnapsenDuoActions::PlayCard(_) => SchnapsenDuoEmptyActions::PlayCard as u8,
            SchnapsenDuoActions::Quit => SchnapsenDuoEmptyActions::Quit as u8,
            SchnapsenDuoActions::SwapTrump(_) => SchnapsenDuoEmptyActions::SwapTrump as u8,
            SchnapsenDuoActions::CloseTalon => SchnapsenDuoEmptyActions::CloseTalon as u8,
            SchnapsenDuoActions::Announce20(_) => SchnapsenDuoEmptyActions::Announce20 as u8,
            SchnapsenDuoActions::Announce40 => SchnapsenDuoEmptyActions::Announce40 as u8,
            SchnapsenDuoActions::DrawCard => SchnapsenDuoEmptyActions::DrawCard as u8,
            SchnapsenDuoActions::CuttDeck(_) => SchnapsenDuoEmptyActions::CuttDeck as u8,
            SchnapsenDuoActions::TakeCards(_) => SchnapsenDuoEmptyActions::TakeCards as u8,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, FromPrimitive)]
#[repr(u8)]
pub enum SchnapsenDuoEmptyActions {
    PlayCard = 0,
    #[default]
    Quit = 1,
    Announce20 = 20,
    Announce40 = 40,
    SwapTrump = 3,
    CloseTalon = 4,
    DrawCard = 5,
    CuttDeck = 6,
    TakeCards = 7,
}

impl From<SchnapsenDuoActions> for SchnapsenDuoEmptyActions {
    fn from(action: SchnapsenDuoActions) -> Self {
        match action {
            SchnapsenDuoActions::PlayCard(_) => SchnapsenDuoEmptyActions::PlayCard,
            SchnapsenDuoActions::Quit => SchnapsenDuoEmptyActions::Quit,
            SchnapsenDuoActions::SwapTrump(_) => SchnapsenDuoEmptyActions::SwapTrump,
            SchnapsenDuoActions::CloseTalon => SchnapsenDuoEmptyActions::CloseTalon,
            SchnapsenDuoActions::Announce20(_) => SchnapsenDuoEmptyActions::Announce20,
            SchnapsenDuoActions::Announce40 => SchnapsenDuoEmptyActions::Announce40,
            SchnapsenDuoActions::DrawCard => SchnapsenDuoEmptyActions::DrawCard,
            SchnapsenDuoActions::CuttDeck(_) => SchnapsenDuoEmptyActions::CuttDeck,
            SchnapsenDuoActions::TakeCards(_) => SchnapsenDuoEmptyActions::TakeCards,
        }
    }
}

impl PartialEq<SchnapsenDuoEmptyActions> for SchnapsenDuoActions {
    fn eq(&self, other: &SchnapsenDuoEmptyActions) -> bool {
        self.clone().into_int() == other.clone() as u8
    }
}

impl SchnapsenDuoEmptyActions {
    pub const fn event_name(&self) -> &'static str {
        match self {
            SchnapsenDuoEmptyActions::PlayCard => "play_card",
            SchnapsenDuoEmptyActions::Quit => "quit",
            SchnapsenDuoEmptyActions::Announce20 => "announce_20",
            SchnapsenDuoEmptyActions::Announce40 => "announce_40",
            SchnapsenDuoEmptyActions::SwapTrump => "swap_trump",
            SchnapsenDuoEmptyActions::CloseTalon => "close_talon",
            SchnapsenDuoEmptyActions::DrawCard => "draw_card",
            SchnapsenDuoEmptyActions::CuttDeck => "cutt_deck",
            SchnapsenDuoEmptyActions::TakeCards => "take_cards",
        }
    }
}

pub struct SchnapsenDuoTranslator<T>
where
    T: Fn(SchnapsenDuoActions) -> () + Send + 'static,
{
    callbacks: Mutex<Vec<T>>,
    socket: Arc<tokio::sync::Mutex<SocketRef>>,
}

impl<T> SchnapsenDuoTranslator<T>
where
    T: Fn(SchnapsenDuoActions) -> () + Send + 'static,
{
    pub async fn listen(socket: Arc<tokio::sync::Mutex<SocketRef>>) -> Arc<Self> {
        let new = Arc::new(Self {
            callbacks: Mutex::new(Vec::new()),
            socket
        });
        new.clone().init_events().await;
        debug!("Initialized events");
        new
    }


    async fn init_events(self: Arc<Self>) {
        debug!("Initializing events");
        let socket = self.socket.lock().await;
        let clone = self.clone();

        socket.on(
            SchnapsenDuoEmptyActions::PlayCard.event_name(),
            move |Data(data): Data<Card>| async move { clone.notify(SchnapsenDuoActions::PlayCard(data))},
        );
        debug!("Initialized play_card");
        let clone = self.clone();
        socket.on(SchnapsenDuoEmptyActions::Quit.event_name(), move || {
            clone.notify(SchnapsenDuoActions::Quit)
        }); // NOTE: This is only left in as an example. Remove!
        let clone = self.clone();
        socket.on(
            SchnapsenDuoEmptyActions::SwapTrump.event_name(),
            move |Data(data): Data<Card>| clone.notify(SchnapsenDuoActions::SwapTrump(data)),
        );
        let clone = self.clone();
        socket.on(
            SchnapsenDuoEmptyActions::CloseTalon.event_name(),
            move || clone.notify(SchnapsenDuoActions::CloseTalon),
        );
        let clone = self.clone();
        socket.on(
            SchnapsenDuoEmptyActions::Announce20.event_name(),
            move |Data(data)| clone.notify(SchnapsenDuoActions::Announce20(data)),
        );
        let clone = self.clone();
        socket.on(
            SchnapsenDuoEmptyActions::Announce40.event_name(),
            move || clone.notify(SchnapsenDuoActions::Announce40),
        );
        let clone = self.clone();
        socket.on(SchnapsenDuoEmptyActions::DrawCard.event_name(), move || {
            clone.notify(SchnapsenDuoActions::DrawCard)
        });
        let clone = self.clone();
        socket.on(SchnapsenDuoEmptyActions::CuttDeck.event_name(), move |Data(idx): Data<usize>| {
            clone.notify(SchnapsenDuoActions::CuttDeck(idx))
        });
        let clone = self.clone();
        socket.on(
            SchnapsenDuoEmptyActions::TakeCards.event_name(),
            move |Data(data): Data<usize>| clone.notify(SchnapsenDuoActions::TakeCards(data)),
        );
    }

    pub fn on_event(&self, callback: T) {
        self.callbacks.lock().unwrap().push(callback);
    }

    fn notify(&self, action: SchnapsenDuoActions) {
        for callback in self.callbacks.lock().unwrap().iter() {
            callback(action.clone());
        }
    }
}
