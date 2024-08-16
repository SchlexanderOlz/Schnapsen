use std::{collections::HashMap, sync::{Arc, Mutex}};

use crate::translator::{SchnapsenDuoActions, SchnapsenDuoEmptyActions};

type Schnapsen = ();

#[derive(Clone, PartialEq)]
pub struct Player {
    pub write: String,
}

pub struct Performer<'a>
{
    instance: Arc<Mutex<Schnapsen>>,
    player: &'a Player,
    functions: HashMap<SchnapsenDuoEmptyActions, Box<dyn Fn(&mut Schnapsen, &'a Player, SchnapsenDuoActions) -> () + Sync + Send + 'a>>,
}

impl<'a> Performer<'a>
{
    pub fn new(player: &'a Player, instance: Arc<Mutex<Schnapsen>>) -> Self {
        let function: Box<dyn Fn(&mut Schnapsen, &'a Player, SchnapsenDuoActions) + Send + Sync + 'a> = Box::new(|instance: &mut Schnapsen, player: &'a Player, action: SchnapsenDuoActions| {
            match action {
                SchnapsenDuoActions::PlayCard(_) => {
                    println!("Play card");
                }
                _ => panic!("Invalid action"),
            }
        });

        let mut functions = HashMap::new();
        functions.insert(SchnapsenDuoEmptyActions::PlayCard, function);

        Self {
            instance,
            functions,
            player
        }
    }

    pub fn perform(&self, action: SchnapsenDuoActions) {
        self.functions.get(&action.clone().into()).unwrap()(&mut self.instance.lock().unwrap(), self.player, action);
    }
}
