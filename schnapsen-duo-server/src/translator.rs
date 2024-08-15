use std::sync::{Arc, Mutex};

use schnapsen_rs::models::Card;
use socketioxide::{adapter::LocalAdapter, extract::{Data, SocketRef}, handler::MessageHandler};

#[derive(Clone)]
pub enum SchnapsenDuoActions {
    PlayCard(Card),
    Quit
}

impl SchnapsenDuoActions {
    pub const fn into_int(self) -> u8 
    {
        match self {
            SchnapsenDuoActions::PlayCard(_) => SchnapsenDuoEmptyActions::PlayCard as u8,
            SchnapsenDuoActions::Quit => SchnapsenDuoEmptyActions::Quit as u8
        }
    }
}


#[derive(Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SchnapsenDuoEmptyActions {
    PlayCard = 0,
    Quit = 1
}

impl PartialEq<SchnapsenDuoEmptyActions> for SchnapsenDuoActions {
    fn eq(&self, other: &SchnapsenDuoEmptyActions) -> bool {
        self.clone().into_int() == other.clone() as u8
    }
}

impl From<SchnapsenDuoActions> for SchnapsenDuoEmptyActions {
    fn from(action: SchnapsenDuoActions) -> Self {
        SchnapsenDuoEmptyActions::from_int(action.into_int())
    }
}

impl SchnapsenDuoEmptyActions {
    pub const fn from_int(val: u8) -> Self 
    {
        match val {
            0 => SchnapsenDuoEmptyActions::PlayCard,
            1 => SchnapsenDuoEmptyActions::Quit,
            _ => panic!("Invalid value")
        }
    }
}



pub struct SchnapsenDuoTranslator<T> 
where T: Fn(SchnapsenDuoActions) -> () + Sync + Send + 'static
{
    callbacks: Mutex<Vec<T>>
}

impl<T> SchnapsenDuoTranslator<T> 
where T: Fn(SchnapsenDuoActions) -> () + Sync + Send + 'static
{
    pub fn listen(socket: SocketRef) -> Arc<Self>
    {
        let new = Arc::new(Self {
            callbacks: Mutex::new(Vec::new())
        });

        let clone = new.clone();
        socket.on("play_card".to_string(), move |Data(data): Data<Card>| clone.notify(SchnapsenDuoActions::PlayCard(data)) );
        let clone = new.clone();
        socket.on("quit", move || clone.notify(SchnapsenDuoActions::Quit) ); // NOTE: This is only left in as an example. Remove!
        new
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
