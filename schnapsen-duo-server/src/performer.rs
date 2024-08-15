use std::collections::HashMap;

use crate::translator::{SchnapsenDuoActions, SchnapsenDuoEmptyActions};

type Schnapsen = ();

pub struct Performer<T>
where
    T: Fn(SchnapsenDuoActions) -> () + Sync + Send + 'static,
{
    instance: Schnapsen,
    functions: HashMap<SchnapsenDuoEmptyActions, T>,
}

impl<T> Performer<T>
where
    T: Fn(SchnapsenDuoActions) -> () + Sync + Send + 'static,
{
    pub fn new(instance: Schnapsen) -> Self {
        Self {
            instance,
            functions: HashMap::new(),
        }
    }

    pub fn perform(&self, action: SchnapsenDuoActions) {
        self.functions.get(&action.clone().into()).unwrap()(action);
    }
}
