use std::sync::{Arc, Mutex, RwLock};

use crate::{
    models::{Announcement, Card, Player},
    PlayerError, SchnapsenDuo,
};

pub struct SchnapsenDuoClient {
    player: Arc<RwLock<Player>>,
    instance: Arc<Mutex<SchnapsenDuo>>,
}

impl SchnapsenDuoClient {
    pub fn new(player: Arc<RwLock<Player>>, instance: Arc<Mutex<SchnapsenDuo>>) -> Self {
        Self { player, instance }
    }

    #[inline]
    pub fn cutt_deck(&self, cards_to_take: usize) -> Result<(), crate::PlayerError> {
        self.instance
            .lock()
            .unwrap()
            .cutt_deck(self.player.clone(), cards_to_take)
    }

    #[inline]
    pub fn get_player_id(&self) -> String {
        self.player.read().unwrap().id.clone()
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.instance
            .lock()
            .unwrap()
            .is_active(&self.player.read().unwrap())
    }

    pub fn draw_card(&self) -> Result<(), crate::PlayerError> {
        let card = self
            .instance
            .lock()
            .unwrap()
            .draw_card_after_trick(self.player.clone())?;
        self.player.write().unwrap().cards.push(card);

        self.update_player_state();
        Ok(())
    }

    pub fn close_talon(&self) -> Result<(), crate::PlayerError> {
        self.instance
            .lock()
            .unwrap()
            .close_talon(self.player.clone())
    }

    pub fn take_cards_til(&self, idx: usize) -> Result<(), crate::PlayerError> {
        let cards = self
            .instance
            .lock()
            .unwrap()
            .take_cards_til(self.player.clone(), idx)?;

        self.player.write().unwrap().cards.extend(cards);
        Ok(())
    }

    pub fn play_card(&self, card: crate::Card) -> Result<(), crate::PlayerError> {
        self.instance
            .lock()
            .unwrap()
            .play_card(self.player.clone(), card.clone())?;
        Ok(())
    }

    pub fn swap_trump(&self, card: crate::Card) -> Result<(), crate::PlayerError> {
        let swap = self
            .instance
            .lock()
            .unwrap()
            .swap_trump(self.player.clone(), card.clone())?;

        self.player.write().unwrap().cards.retain(|x| *x != card);
        self.player.write().unwrap().cards.push(swap);

        self.update_player_state();

        Ok(())
    }

    pub fn announce_40(&self) -> Result<(), crate::PlayerError> {
        let announcement = self
            .instance
            .lock()
            .unwrap()
            .announce_40(&self.player.read().unwrap())?;

        self.announce_state_changes(announcement)?;
        Ok(())
    }

    pub fn announce_20(&self, cards: [Card; 2]) -> Result<(), crate::PlayerError> {
        let announcement = self
            .instance
            .lock()
            .unwrap()
            .announce_20(&self.player.read().unwrap(), cards)?;

        self.announce_state_changes(announcement)?;
        Ok(())
    }

    fn announce_state_changes(&self, announcement: Announcement) -> Result<(), PlayerError> {
        self.player
            .write()
            .unwrap()
            .announcements
            .push(announcement.clone());

        self.player
            .write()
            .unwrap()
            .announcable
            .retain(|x| *x == announcement);

        let mut instance_lock = self.instance.lock().unwrap();
        instance_lock
            .notify_changes_playable_cards(&self.player.read().unwrap(), &announcement.cards);

        self.player.try_write().unwrap().playable_cards = announcement.cards.to_vec();
        instance_lock.update_announcable_props(self.player.clone());
        instance_lock.update_finish_round(self.player.clone())?;
        Ok(())
    }

    fn update_player_state(&self) {
        let instance_lock = self.instance.lock().unwrap();
        let (_, announcable) = instance_lock.notify_announcable_props(&self.player.read().unwrap());
        self.player.try_write().unwrap().announcable = announcable;

        let (_, can_swap) = instance_lock.notify_swap_trump_check(&self.player.read().unwrap());
        self.player.try_write().unwrap().possible_trump_swap = can_swap;

        let playable_cards = instance_lock.find_playable_cards(self.player.clone());
        instance_lock.notify_changes_playable_cards(&self.player.read().unwrap(), &playable_cards);

        self.player.try_write().unwrap().playable_cards = playable_cards.to_vec();
    }
}
