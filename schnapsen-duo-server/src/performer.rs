use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};

use schnapsen_rs::{client::SchnapsenDuoClient, SchnapsenDuo};
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
            &SchnapsenDuoClient,
            SchnapsenDuoActions,
        ) -> Result<(), PerformerError>
        + Send
        + Sync
        + 'a,
>;

pub struct Performer<'a> {
    client: SchnapsenDuoClient,
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
        
        let client = SchnapsenDuoClient::new(player.clone(), instance.clone());

        Self {
            client,
            functions,
        }
    }

    pub fn perform(&self, action: SchnapsenDuoActions) -> Result<(), PerformerError> {
        debug!(
            "Performing action: {:?} by player: {:?}",
            action,
            self.client.get_player_id()
        );
        let res = self.functions.get(&action.clone().into()).unwrap()(
            &self.client,
            action.clone(),
        );
        if res.is_err() {
            debug!(
                "Error performing action: {:?} by player: {:?}",
                action,
                self.client.get_player_id()
            );
        } else {
            debug!(
                "Successfully performed action: {:?} by player: {:?}",
                action,
                self.client.get_player_id()
            );
        }
        res
    }

    fn play_card(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::PlayCard(card) = action {
            return Ok(client.play_card(card)?);
        }
        Err(PerformerError::CallError)
    }

    fn quit(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Err(PerformerError::CallError) // TODO: Implement function
    }

    fn swap_trump(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::SwapTrump(card) = action {
            return Ok(client.swap_trump(card)?);
        }
        Err(PerformerError::CallError)
    }

    fn close_talon(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::CloseTalon = action {
            return Ok(client.close_talon()?);
        }
        Err(PerformerError::CallError)
    }

    fn announce_20(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::Announce20(cards) = action {
            return Ok(client.announce_20(cards)?);
        }
        Err(PerformerError::CallError)
    }

    fn announce_40(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        Ok(client.announce_40()?)
    }

    fn cutt_deck(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::CuttDeck(idx) = action {
            return Ok(client.cutt_deck(idx)?);
        }
        Err(PerformerError::CallError)
    }

    fn take_cards(
        client: &SchnapsenDuoClient,
        action: SchnapsenDuoActions,
    ) -> Result<(), PerformerError> {
        if let SchnapsenDuoActions::TakeCards(cards) = action {
            return Ok(client.take_cards_til(cards)?); // TODO: This might lead to concurrency issues
        }
        Err(PerformerError::CallError)
    }
}
