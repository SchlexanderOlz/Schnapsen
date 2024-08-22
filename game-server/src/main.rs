use listener::{MatchCreated, ModeServerMatchCreated};
use serde::Serialize;
use tracing::{debug, info};
use tracing_subscriber::FmtSubscriber;

mod games;
mod listener;

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
    tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
    info!("Starting Server");
    let app = axum::Router::new();
    let router = listener::listen(app, on_new_match);

    let public_addr = std::env::var("PUBLIC_ADDR").expect("PUBLIC_ADDR must be set");
    let server_info = GameServer {
        name: "Schnapsen".to_string(),
        modes: vec![GameMode {
            name: "duo".to_string(),
            player_count: 2,
            computer_lobby: false,
        }],
        server: public_addr,
        token: "token".to_string(),
    };
    let client = reqwest::Client::new();
    let url = std::env::var("GAME_REGISTER_URL").expect("GAME_REGISTER_URL must be set");
    client
        .post(url.as_str())
        .json(&server_info)
        .send()
        .await
        .unwrap();
    info!("Registered Game at {url}");

    let addr = std::env::var("HOST_ADDR").expect("HOST_ADDR must be set");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

async fn on_new_match(new_match: listener::CreateMatch) -> MatchCreated {
    // TODO: Dynamically create the match instead of always Schnapsen Duo
    debug!("Creating new match: {:?}", new_match);
    let create_url =
        std::env::var("SCHNAPSEN_DUO_CREATE_URL").expect("SCHNAPSEN_DUO_CREATE_URL must be set");
    let client = reqwest::Client::new();
    let response: ModeServerMatchCreated = client
        .post(create_url.as_str()) // TODO: Get the actuall port of the Mode-Server from some in-memory store
        .json(&new_match)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // TODO: The created match should actually be added to some state-management of active matches. This server is just a simple temporary implementation.
    let match_addr = std::env::var("SCHNAPSEN_DUO_PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");

    MatchCreated {
        player_write: response.player_write,
        read: response.read,
        url: match_addr,
    }
}
