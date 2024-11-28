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
            .cutt_deck(&self.player.read().unwrap(), cards_to_take)
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
            .draw_card_after_trick(&self.player.read().unwrap())?;
        self.player.write().unwrap().cards.push(card);

        let instance_lock = self.instance.lock().unwrap();

        let mut player_lock = self.player.write().unwrap();
        instance_lock.update_announcable_props(&mut player_lock);
        instance_lock.run_swap_trump_check(&mut player_lock);
        instance_lock.update_playable_cards(&mut player_lock);

        Ok(())
    }

    pub fn close_talon(&self) -> Result<(), crate::PlayerError> {
        self.instance
            .lock()
            .unwrap()
            .close_talon(&self.player.read().unwrap())
    }

    pub fn take_cards_til(&self, idx: usize) -> Result<(), crate::PlayerError> {
        let cards = self
            .instance
            .lock()
            .unwrap()
            .take_cards_til(&self.player.read().unwrap(), idx)?;

        self.player.write().unwrap().cards.extend(cards);
        Ok(())
    }

    pub fn play_card(&self, card: crate::Card) -> Result<(), crate::PlayerError> {
        let mut instance_lock = self.instance.lock().unwrap();

        let cards = instance_lock
            .play_card(&self.player.read().unwrap(), card.clone())?;
        self.player.write().unwrap().cards.retain(|x| *x != card);

        if let Some(trick) = cards {
            self
                .player
                .write()
                .unwrap()
                .tricks
                .push(trick);

            instance_lock.update_finish_round(self.player.clone())?;
        }

        self.player
            .write()
            .unwrap()
            .playable_cards
            .retain(|x| *x != card);


        let mut player_lock = self.player.write().unwrap();
        instance_lock.update_announcable_props(&mut player_lock);
        instance_lock.run_swap_trump_check(&mut player_lock);
        instance_lock.update_playable_cards(&mut player_lock);

        Ok(())
    }

    pub fn swap_trump(&self, card: crate::Card) -> Result<(), crate::PlayerError> {
        let swap = self
            .instance
            .lock()
            .unwrap()
            .swap_trump(&self.player.read().unwrap(), card.clone())?;
        self.player.write().unwrap().cards.retain(|x| *x != card);
        self.player.write().unwrap().cards.push(swap);

        self.instance
            .lock()
            .unwrap()
            .update_playable_cards(&mut self.player.write().unwrap());
        Ok(())
    }

    pub fn announce_40(&self) -> Result<(), crate::PlayerError> {
        let announcement = self
            .instance
            .lock()
            .unwrap()
            .announce_40(&self.player.read().unwrap())?;
        self.player
            .write()
            .unwrap()
            .announcements
            .push(announcement.clone());

        self.announce_state_changes(announcement)?;
        Ok(())
    }

    pub fn announce_20(&self, cards: [Card; 2]) -> Result<(), crate::PlayerError> {
        let announcement = self
            .instance
            .lock()
            .unwrap()
            .announce_20(&self.player.read().unwrap(), cards)?;
        self.player
            .write()
            .unwrap()
            .announcements
            .push(announcement.clone());
        self.announce_state_changes(announcement)?;
        Ok(())
    }

    // NOTE: Calls like these directly on the instance are not recommended as they modify the player object. This should rather be done directly in this class.
    fn announce_state_changes(&self, announcement: Announcement) -> Result<(), PlayerError> {
        let mut instance_lock = self.instance.lock().unwrap();
        instance_lock
            .notify_changes_playable_cards(&self.player.read().unwrap(), &announcement.cards);
        instance_lock.update_announcable_props(&mut self.player.write().unwrap());
        instance_lock.update_finish_round(self.player.clone())?;
        Ok(())
    }
}
