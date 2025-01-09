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
    events: Vec<(Option<String>, TimedEvent<T>)>,
}

impl<T> EventLogger<T>
where
    T: EventLike,
{
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn log(&mut self, event: TimedEvent<T>, user_id: Option<String>) {
        for (i, prev_event) in self.events.iter().rev().enumerate() {
            if prev_event.1.timestamp <= event.timestamp {
                self.events.insert(self.events.len() - i, (user_id, event));
                return;
            }
        }

        self.events.push((user_id, event));
    }

    pub fn get(&self, timestamp: u64) -> Option<&TimedEvent<T>> {
        self.events
            .binary_search_by_key(&timestamp, |event| event.1.timestamp)
            .ok()
            .map(|idx| &self.events[idx].1)
    }

    pub fn events_since(&self, timestamp: u64, user_id: Option<String>) -> Vec<&TimedEvent<T>> {
        let user_events: Vec<_> = self
            .events
            .iter()
            .filter(|timed_event| timed_event.0.is_none() || timed_event.0 == user_id)
            .collect();

        let find_first = || {
            user_events
                .iter()
                .position(|event| event.1.timestamp >= timestamp)
                .unwrap_or(0)
        };

        let idx = user_events
            .binary_search_by_key(&timestamp, |event| event.1.timestamp)
            .unwrap_or(find_first());

        user_events[idx..].iter().map(|x| &x.1).collect()
    }

    pub fn all(&self) -> Vec<&TimedEvent<T>> {
        self.events.iter().map(|x| &x.1).collect()
    }
}
