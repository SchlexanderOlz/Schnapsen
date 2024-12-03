use rbtree::RBTree;
use serde::Serialize;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc,
};

pub trait EventLike: Hash + PartialEq + Eq {}

#[derive(Debug, PartialEq, Eq)]
pub struct Event<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    next: Option<*const Event<Prv, Pub>>,
    hash: u64,
    value: EventType<Prv, Pub>,
}

impl<Prv, Pub> Event<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    pub fn since(&self) -> Vec<&Event<Prv, Pub>> {
        if let Some(next) = self.next {
            let mut values = unsafe { &*next }.since();
            values.push(self);
            return values;
        }
        return vec![self];
    }
}

impl<Prv, Pub> Ord for Event<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl<Prv, Pub> PartialOrd for Event<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Hash, Debug, PartialEq, Eq)]
pub enum EventType<Prv, Pub> {
    Private(Prv),
    Public(Pub),
}

impl<Prv, Pub> From<EventType<Prv, Pub>> for Event<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    fn from(value: EventType<Prv, Pub>) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        let hash = hasher.finish();

        Event {
            hash,
            value,
            next: None,
        }
    }
}

pub struct EventLogger<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    events: RBTree<u64, Event<Prv, Pub>>,
    prev: Option<*mut Event<Prv, Pub>>,
    root: Option<*const Event<Prv, Pub>>,
}

impl<'a, Prv, Pub> EventLogger<Prv, Pub>
where
    Prv: EventLike,
    Pub: EventLike,
{
    pub fn new() -> Self {
        Self {
            events: RBTree::new(),
            prev: None,
            root: None,
        }
    }

    pub fn log(&'a mut self, event: EventType<Prv, Pub>) {
        let mut event: Event<Prv, Pub> = event.into();

        let event_ptr: *mut _ = &mut event;

        if let Some(prev) = self.prev.replace(event_ptr) {
            unsafe {
                (*prev).next = Some(event_ptr);
            }
        } else {
            self.root = Some(event_ptr);
        }

        self.events.insert(event.hash, event);
    }

    pub fn get(&self, hash: u64) -> Option<&Event<Prv, Pub>> {
        self.events.get(&hash)
    }

    pub fn events_since(&self, hash: u64) -> Vec<&Event<Prv, Pub>> {
        self.events.get(&hash).map(|event| event.since()).unwrap_or_default()
    }

    pub fn all(&self) -> Vec<&Event<Prv, Pub>> {
        self.root.map(|root| unsafe { &*root }.since()).unwrap_or_default()
    }
}
