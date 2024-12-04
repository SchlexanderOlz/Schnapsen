use rbtree::RBTree;
use serde::Serialize;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc,
};

pub trait EventLike: Hash + PartialEq + Eq {}

#[derive(Debug, PartialEq, Eq)]
pub struct Event<T>
where T: EventLike
{
    next: Option<*const Event<T>>,
    hash: u64,
    value: T,
}

impl<T> Event<T>
where T: EventLike
{
    pub fn new(value: T) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        let hash = hasher.finish();

        Self {
            next: None,
            hash,
            value,
        }
    }
}

impl<T> Event<T>
where T: EventLike
{
    pub fn since(&self) -> Vec<&Event<T>> {
        if let Some(next) = self.next {
            let mut values = unsafe { &*next }.since();
            values.push(self);
            return values;
        }
        return vec![self];
    }
}

impl<T> Ord for Event<T>
where T: EventLike
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl<T> PartialOrd for Event<T>
where T: EventLike
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}



pub struct EventLogger<T>
where T: EventLike
{
    events: RBTree<u64, Event<T>>,
    prev: Option<*mut Event<T>>,
    root: Option<*const Event<T>>,
}

impl<T> EventLogger<T>
where T: EventLike + Into<Event<T>>
{
    pub fn new() -> Self {
        Self {
            events: RBTree::new(),
            prev: None,
            root: None,
        }
    }

    pub fn log(&mut self, event: T) {
        let mut event: Event<T> = event.into();

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

    pub fn get(&self, hash: u64) -> Option<&Event<T>> {
        self.events.get(&hash)
    }

    pub fn events_since(&self, hash: u64) -> Vec<&Event<T>> {
        self.events.get(&hash).map(|event| event.since()).unwrap_or_default()
    }

    pub fn all(&self) -> Vec<&Event<T>> {
        self.root.map(|root| unsafe { &*root }.since()).unwrap_or_default()
    }
}
