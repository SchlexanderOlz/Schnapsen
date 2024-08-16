use std::sync::{Arc, Mutex};

use listener::MatchCreated;
use axum;
use socketioxide::extract::{Data, SocketRef};

mod listener;
mod translator;
mod performer;

const DEFAULT_URL: &str = "0.0.0.0:6000"; // TODO: Set the default URL correctly at some point and register it at the game-server



#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let router = axum::Router::new().layer(layer);

    let on_create = move |new_match: listener::CreateMatch| {
        let io = io.clone();
        let instance = Arc::new(Mutex::new(()));

        async move {
            // TODO: Create the duo schnapsen game instance

            let read = "read".to_string();

            let instance_clone = instance.clone();
            io.ns(format!("/{read}"), move |socket: SocketRef| {
                let player = performer::Player { write: format!("write") };

                let translator = translator::SchnapsenDuoTranslator::listen(socket);
                translator.on_event(move |action| {
                    let performer = performer::Performer::new(&player, instance_clone.clone());
                    performer.perform(action);
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
    let router = listener::listen(router, on_create).await;
    axum::serve(listener, router).await.unwrap();
}
