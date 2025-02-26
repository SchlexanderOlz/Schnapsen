use helpers::has_no_repeating_elements;

use super::*;

#[test]
fn test_create_instance() {
    let mut instance = SchnapsenDuo::new(&["1".to_string(), "2".to_string()]);
    assert!(has_no_repeating_elements(instance.deck.iter()));
    assert_eq!(instance.deck.len(), 20);
    assert_eq!(instance.players.len(), 2);

    instance.set_active_player(instance.players[0].clone()).unwrap();
    instance.distribute_cards().unwrap();

    let player1 = instance.players[0].read().unwrap();
    let player2 = instance.players[1].read().unwrap();

    assert_eq!(player1.cards.len(), 5);
    assert!(has_no_repeating_elements(player1.cards.iter()));
    assert_eq!(player2.cards.len(), 5);
    assert!(has_no_repeating_elements(player2.cards.iter()));
    assert!(instance.closed_talon.is_none());
}

#[test]
fn test_play_card_allowed() {}

pub mod helpers {
    use std::{collections::HashSet, hash::Hash};

    use crate::models::{Card, CardSuit, CardVal};

    impl Hash for Card {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.value.hash(state);
            self.suit.hash(state);
        }
    }

    impl Hash for CardVal {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write_u8(self.clone() as u8);
        }
    }

    impl Hash for CardSuit {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write_u8(self.clone() as u8);
        }
    }

    pub fn has_no_repeating_elements<T>(iter: T) -> bool
    where
        T: IntoIterator,
        T::Item: Eq + Hash,
    {
        let mut uniq = HashSet::new();
        iter.into_iter().all(move |x| uniq.insert(x))
    }
}
