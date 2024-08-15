use listener::{MatchCreated, ModeServerMatchCreated};

mod listener;
mod games;


const SERVER_IP: &str = "127.0.0.1";

#[derive(Debug)]
struct AuthError;
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AuthError")
    }
}
impl std::error::Error for AuthError {}


fn main() {
    tokio::task::spawn(listener::listen(on_new_match));
}

async fn on_new_match(new_match: listener::CreateMatch) -> MatchCreated {
    let client = reqwest::Client::new();
    let response: ModeServerMatchCreated = client.post(format!("http://{}:{}", SERVER_IP, 5000)) // TODO: Get the actuall port of the Mode-Server from some in-memory store
        .json(&new_match)
        .send()
        .await
        .unwrap().json().await.unwrap();

    // TODO: The created match should actually be added to some state-management of active matches. This server is just a simple temporary implementation.

    MatchCreated {
        player_write: response.player_write,
        read: response.read,
        url: format!("{}:{}", SERVER_IP, 5000)
    }
}
