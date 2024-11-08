use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct GameServer {
    pub region: String,
    pub game: String,
    pub mode: GameMode,
    pub server_priv: String,
    pub server_pub: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}


#[derive(Serialize, Deserialize, Debug)]
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



#[derive(Serialize, Debug)]
pub struct MatchCreated {
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: GameMode,
    pub read: String,
    pub url_pub: String,
    pub url_priv: String,
    pub region: String,
}