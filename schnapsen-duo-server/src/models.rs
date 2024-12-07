use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{
    emitter::EventIdentifier, events::{self, TimedEvent},
};

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
    pub mode: String,
    pub server_priv: String,
    pub server_pub: String,
    pub min_players: u32,
    pub max_players: u32,
    pub ranking_conf: RankingConf,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateMatch {
    pub game: String,
    pub players: Vec<String>,
    pub ai_players: Vec<String>,
    pub mode: String,
    pub ai: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct MatchCreated {
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: String,
    pub ai: bool,
    pub ai_players: Vec<String>,
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
pub struct Ranking {
    pub performances: HashMap<String, Vec<String>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct MatchResult {
    pub match_id: String,
    pub winners: HashMap<String, u8>,
    pub losers: HashMap<String, u8>,
    pub ranking: Ranking,
    pub event_log: Vec<TimedEvent<events::SchnapsenDuoEventType>>
}
