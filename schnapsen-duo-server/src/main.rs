use std::sync::Arc;

use listener::MatchCreated;
use axum;
use socketioxide::extract::{Data, SocketRef};

mod listener;
mod translator;
mod performer;

const DEFAULT_URL: &str = "127.0.0.1:5000"; // TODO: Set the default URL correctly at some point and register it at the game-server

fn setup_handlers(socket: SocketRef) {
    socket.on("saus", || {
    })
}


#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let router = axum::Router::new().layer(layer);

    let on_create = move |new_match: listener::CreateMatch| {
        let io = io.clone();
        async move {
            // TODO: Create the duo schnapsen game instance
            let read = "read".to_string();
            io.ns(format!("/{read}"), setup_handlers);
            // TODO: The created match should be added to some active-state
            MatchCreated {
                player_write: new_match.players.iter().enumerate().map(|(i, player)| (player.clone(), format!("write{i}"))).collect(),
                read: "read".to_string(),
            }
        }
    };
    let router = listener::listen(router, on_create).await;
    axum::serve(listener, router).await.unwrap();
}
