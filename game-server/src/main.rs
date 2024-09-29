use std::{thread, time::Duration};

use serde::Serialize;
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;

mod games;

#[derive(Serialize)]
pub struct GameServer {
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server_priv: String,
    pub server_pub: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}

#[derive(Serialize)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

#[async_recursion::async_recursion]
async fn try_connect(url: &str, server_info: &GameServer) {
    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .json(server_info)
        .send()
        .await
        .unwrap();

    debug!("{:?}", res);

    if let Err(err) = res.error_for_status() {
        error!("Game could not be registered. Err: {}", err);
        thread::sleep(Duration::from_secs(2));
        try_connect(url, server_info).await;
    } else {
        info!("Registered Game at {url}");
    }
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Starting Server");

    let public_addr = std::env::var("PUBLIC_ADDR").expect("PUBLIC_ADDR must be set");
    let private_addr = std::env::var("PRIVATE_ADDR").expect("PRIVATE_ADDR must be set");
    let server_info = GameServer {
        name: "Schnapsen".to_string(),
        modes: vec![GameMode {
            name: "duo".to_string(),
            player_count: 2,
            computer_lobby: false,
        }],
        server_pub: public_addr,
        server_priv: private_addr,
        token: "token".to_string(),
    };
    let url = std::env::var("GAME_REGISTER_URL").expect("GAME_REGISTER_URL must be set");

    try_connect(url.as_str(), &server_info).await;
}
