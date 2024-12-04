use std::{collections::HashMap, hash::{Hash, Hasher}};

use serde::{Deserialize, Serialize};

use crate::event_logger::{Event, EventLike};

#[derive(Serialize)]
pub struct Performance {
    pub name: String,
    pub weight: i32,
}

#[derive(Serialize)]
pub struct RankingConf {
    pub max_stars: i32,
    pub description: String,
    pub performances: Vec<Performance>,
}


#[derive(Serialize)]
pub struct GameServer {
    pub region: String,
    pub game: String,
    pub mode: GameMode,
    pub server_priv: String,
    pub server_pub: String,
    pub token: String, // Token to authorize as the main-server at this game-server

    pub ranking_conf: RankingConf,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}


#[derive(Deserialize, Debug)]
pub struct CreateMatch {
    pub game: String,
    pub players: Vec<String>,
    pub mode: GameMode,
}



#[derive(Serialize, Debug, Clone)]
pub struct MatchCreated {
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: GameMode,
    pub read: String,
    pub url_pub: String,
    pub url_priv: String,
    pub region: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Timeout {
    pub user_id: String,
    pub reason: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct MatchResult {
    pub match_id: String,
    pub winner: String,
    pub points: u8,
    pub ranked: HashMap<String, u8>,
}




#[derive(Serialize, Hash, Debug, PartialEq, Eq)]
pub enum EventType<Prv, Pub> {
    Private(Prv),
    Public(Pub),
}

impl<Prv, Pub> EventLike for EventType<Prv, Pub>
where Prv: EventLike, Pub: EventLike {}

impl<Prv, Pub> From<EventType<Prv, Pub>> for Event<EventType<Prv, Pub>>
where
    Prv: EventLike,
    Pub: EventLike,
{
    fn from(value: EventType<Prv, Pub>) -> Self {
        Event::new(value)
    }
}