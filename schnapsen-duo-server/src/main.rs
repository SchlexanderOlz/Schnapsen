use std::sync::{Arc, Mutex};

use listener::MatchCreated;
use axum;
use schnapsen_rs::SchnapsenDuo;
use socketioxide::extract::{Data, SocketRef};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

mod listener;
mod translator;
mod performer;

const DEFAULT_URL: &str = "0.0.0.0:6060"; // TODO: Set the default URL correctly at some point and register it at the game-server



#[tokio::main]
async fn main() {
    tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
    info!("Starting Schnapsen Duo Server");
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let router = axum::Router::new().layer(layer);

    let on_create = move |new_match: listener::CreateMatch| {
        let player_ids = vec!["write0".to_string(), "write1".to_string()];

        info!("Creating new match: {:?}", new_match);
        let io = io.clone();
        let instance = Arc::new(Mutex::new(SchnapsenDuo::new(player_ids.as_slice().try_into().unwrap())));

        async move {
            // TODO: Create the duo schnapsen game instance

            let read = "read".to_string();

            let instance_clone = instance.clone();
            io.ns(format!("/{read}"), move |socket: SocketRef| {
                let player_id: String = "write1".to_string(); // TODO: Get the player ID from the socket 

                let translator = translator::SchnapsenDuoTranslator::listen(socket.clone());
                let player_id_clone = player_id.clone();
                translator.on_event(move |action| {
                    let performer = performer::Performer::new(player_id_clone.clone(), instance_clone.clone());
                    let res = performer.perform(action);
                    if res.is_err() {
                        socket.emit("error", res.unwrap_err().to_string()).unwrap();
                    }
                });

                // TODO: Set up an event handler in the instance which notifies if the state for player changes. If this socket connect was without a write token, only subscribe to the global state changes in the instance.
            });
            // TODO: The created match should be added to some active-state
            MatchCreated {
                player_write: new_match.players.iter().enumerate().map(|(i, player)| (player.clone(), format!("write{i}"))).collect(),
                read,
            }
        }
    };
    let router = listener::listen(router, on_create);
    info!("Listening on {}", DEFAULT_URL);
    axum::serve(listener, router).await.unwrap();
}
