use listener::{MatchCreated, ModeServerMatchCreated};
use serde::Serialize;

mod listener;
mod games;


const SERVER_IP: &str = "schnapsen-duo-server";

#[derive(Serialize)]
pub struct GameServer {
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}

#[derive(Serialize)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}


#[tokio::main]
async fn main() {
    tokio::task::spawn(listener::listen(on_new_match));

    let server_info = GameServer {
        name: "Schnapsen".to_string(),
        modes: vec![
            GameMode {
                name: "Duo".to_string(),
                player_count: 2,
                computer_lobby: false,
            }
        ],
        server: "http://game-server:6000".to_string(),
        token: "token".to_string(),
    };
    let client = reqwest::Client::new();
    client.post("http://games-agent:4000/register").json(&server_info).send().await.unwrap();
}

async fn on_new_match(new_match: listener::CreateMatch) -> MatchCreated {
    let client = reqwest::Client::new();
    let response: ModeServerMatchCreated = client.post(format!("http://{}:{}", SERVER_IP, 6000)) // TODO: Get the actuall port of the Mode-Server from some in-memory store
        .json(&new_match)
        .send()
        .await
        .unwrap().json().await.unwrap();

    // TODO: The created match should actually be added to some state-management of active matches. This server is just a simple temporary implementation.

    MatchCreated {
        player_write: response.player_write,
        read: response.read,
        url: format!("{}:{}", SERVER_IP, 6000)
    }
}
