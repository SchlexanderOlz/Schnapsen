use serde::Serialize;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

mod games;

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
}
