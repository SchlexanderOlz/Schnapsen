use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};

use schnapsen_rs::SchnapsenDuo;
use thiserror::Error;
use tracing::debug;

use crate::translator::{SchnapsenDuoActions, SchnapsenDuoEmptyActions};

#[derive(Clone, PartialEq)]
pub struct Player {
    pub write: String,
}

#[derive(Error, Debug)]
pub enum PerformerError {
    #[error("Player error: {0}")]
    SchnapsenError(#[from] schnapsen_rs::PlayerError),
    #[error("Function called with invalid Arguments")]
    CallError,
}

type PerformerFunction<'a> = Box<
    dyn Fn(
            Arc<Mutex<SchnapsenDuo>>,
            Rc<RefCell<schnapsen_rs::models::Player>>,
            SchnapsenDuoActions,
        ) -> Result<(), PerformerError>
        + Send
        + Sync
        + 'a,
>;

pub struct Performer<'a> {
    instance: Arc<Mutex<SchnapsenDuo>>,
    player: Rc<RefCell<schnapsen_rs::models::Player>>,
    functions: HashMap<SchnapsenDuoEmptyActions, PerformerFunction<'a>>,
}

impl<'a> Performer<'a> {
    pub fn new(player_id: String, instance: Arc<Mutex<SchnapsenDuo>>) -> Self {
        let mut functions = HashMap::new();
        functions.insert(
            SchnapsenDuoEmptyActions::PlayCard,
            Box::new(Self::play_card) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::Quit,
            Box::new(Self::quit) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::SwapTrump,
            Box::new(Self::swap_trump) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::CloseTalon,
            Box::new(Self::close_talon) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::Announce20,
            Box::new(Self::announce_20) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::Announce40,
            Box::new(Self::announce_40) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::DrawCard,
            Box::new(Self::draw_card) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::CuttDeck,
            Box::new(Self::cutt_deck) as PerformerFunction<'a>,
        );
        functions.insert(
            SchnapsenDuoEmptyActions::TakeCards,
            Box::new(Self::take_cards) as PerformerFunction<'a>,
        );

        let player = instance
            .lock()
            .unwrap()
            .get_player(player_id.as_str())
            .unwrap();

        Self {
            player,
            instance,
            functions,
        }
    }

    pub fn perform(&self, action: SchnapsenDuoActions) -> Result<(), PerformerError> {
        debug!(
            "Performing action: {:?} by player: {:?}",
            action,
            self.player.borrow().id
        );
        let res = self.functions.get(&action.clone().into()).unwrap()(
            self.instance.clone(),
            self.player.clone(),
            action.clone(),
        );
        if res.is_err() {
            debug!(
                "Error performing action: {:?} by player: {:?}",
                action,
                self.player.borrow().id
            );
        } else {
            debug!(
                "Successfully performed action: {:?} by player: {:?}",
                action,
                self.player.borrow().id
            );
        }
        res
    }

    fn play_card(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::PlayCard(card) = action {
            return Ok(instance.lock().unwrap().play_card(player, card)?);
        }
        Err(PerformerError::CallError)
    }

    fn quit(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Err(PerformerError::CallError) // TODO: Implement function
    }

    fn swap_trump(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::SwapTrump(card) = action {
            return Ok(instance.lock().unwrap().swap_trump(player, card)?);
        }
        Err(PerformerError::CallError)
    }

    fn close_talon(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Err(PerformerError::CallError) // TODO: Implement function
    }

    fn announce_20(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::Announce20(cards) = action {
            return Ok(instance.lock().unwrap().announce_20(player, cards)?);
        }
        Err(PerformerError::CallError)
    }

    fn announce_40(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Ok(instance.lock().unwrap().announce_40(player)?)
    }

    fn draw_card(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Ok(instance.lock().unwrap().draw_card_after_trick(player)?)
    }

    fn cutt_deck(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::CuttDeck(idx) = action {
            return Ok(instance.lock().unwrap().cutt_deck(player, idx)?);
        }
        Err(PerformerError::CallError)
    }

    fn take_cards(
        instance: Arc<Mutex<SchnapsenDuo>>,
        player: Rc<RefCell<schnapsen_rs::models::Player>>,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::TakeCards(cards) = action {
            return Ok(instance.lock().unwrap().take_cards_til(player, cards)?); // TODO: This might lead to concurrency issues
        }
        Err(PerformerError::CallError)
    }
}
