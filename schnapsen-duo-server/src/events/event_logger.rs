use rbtree::RBTree;
use serde::Serialize;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc,
};
use tracing_subscriber::fmt::time;

use crate::emitter::EventIdentifier;

use super::TimedEvent;

pub trait EventLike: EventIdentifier + Clone + Serialize + Clone {}


pub struct EventLogger<T>
where
    T: EventLike,
{
    events: Vec<TimedEvent<T>>,
}

impl<T> EventLogger<T>
where
    T: EventLike,
{
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn log(&mut self, event: TimedEvent<T>) {
        for (i, prev_event) in self.events.iter().rev().enumerate() {
            if prev_event.timestamp <= event.timestamp {
                self.events.insert(self.events.len() - i, event);
                return;
            }
        }

        self.events.push(event);
    }

    pub fn get(&self, timestamp: u64) -> Option<&TimedEvent<T>> {
        self.events
            .binary_search_by_key(&timestamp, |event| event.timestamp)
            .ok()
            .map(|idx| &self.events[idx])
    }

    pub fn events_since(&self, timestamp: u64) -> Vec<&TimedEvent<T>> {
        let find_first = || {
            self.events
                .iter()
                .position(|event| event.timestamp >= timestamp)
                .unwrap_or(0)
        };

        let idx = self
            .events
            .binary_search_by_key(&timestamp, |event| event.timestamp)
            .unwrap_or(find_first());

        self.events[idx..].iter().collect()
    }

    pub fn all(&self) -> Vec<&TimedEvent<T>> {
        self.events.iter().collect()
    }
}
