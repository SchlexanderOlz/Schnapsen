use listener::{MatchCreated, ModeServerMatchCreated};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

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
    let handle = tokio::task::spawn(listener::listen(on_new_match));
    tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();


    let server_info = GameServer {
        name: "Schnapsen".to_string(),
        modes: vec![
            GameMode {
                name: "duo".to_string(),
                player_count: 2,
                computer_lobby: false,
            }
        ],
        server: "http://game-server:5050".to_string(),
        token: "token".to_string(),
    };
    let client = reqwest::Client::new();
    client.post("http://games-agent:7000/register").json(&server_info).send().await.unwrap();
    handle.await.unwrap();
}

async fn on_new_match(new_match: listener::CreateMatch) -> MatchCreated {
    info!("Creating new match: {:?}", new_match);
    let client = reqwest::Client::new();
    let response: ModeServerMatchCreated = client.post(format!("http://{}:{}", SERVER_IP, 6060)) // TODO: Get the actuall port of the Mode-Server from some in-memory store
        .json(&new_match)
        .send()
        .await
        .unwrap().json().await.unwrap();

    // TODO: The created match should actually be added to some state-management of active matches. This server is just a simple temporary implementation.

    MatchCreated {
        player_write: response.player_write,
        read: response.read,
        url: format!("{}:{}", SERVER_IP, 6060)
    }
}
