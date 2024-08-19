use std::{hash::{Hash, Hasher}, sync::{Arc, Mutex}};

use axum;
use listener::MatchCreated;
use schnapsen_rs::{PrivateEvent, PublicEvent, SchnapsenDuo};
use socketioxide::{
    extract::{Data, SocketRef},
    socket,
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

mod emitter;
mod listener;
mod performer;
mod translator;

const DEFAULT_URL: &str = "0.0.0.0:6060"; // TODO: Set the default URL correctly at some point and register it at the game-server

fn setup_private_access(player_id: &str, instance: Arc<Mutex<SchnapsenDuo>>, socket: SocketRef) {
    let player = instance.lock().unwrap().get_player(player_id).unwrap();

    let socket_clone = socket.clone();
    instance
        .lock()
        .unwrap()
        .on_priv_event(player, move |event| {
            emitter::to_private_event_emitter(&event, socket_clone.clone())().unwrap()
        });

    let translator = translator::SchnapsenDuoTranslator::listen(socket.clone());
    let player_id_clone = player_id.clone().to_string();
    translator.on_event(move |action| {
        let performer = performer::Performer::new(player_id_clone.clone(), instance.clone());
        let res = performer.perform(action);
        if res.is_err() {
            socket.emit("error", res.unwrap_err().to_string()).unwrap();
        }
    });
}

#[tokio::main]
async fn main() {
    tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
    info!("Starting Schnapsen Duo Server");
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let router = axum::Router::new().layer(layer);

    let on_create = move |new_match: listener::CreateMatch| {
        info!("Creating new match: {:?}", new_match);
        let write = new_match.players.clone();
        let io = io.clone();
        let instance = Arc::new(Mutex::new(SchnapsenDuo::new(
            write.as_slice().try_into().unwrap(),
        )));

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        instance.lock().unwrap().hash(&mut hasher);
        let read = hasher.finish();


        async move {
            let instance_clone = instance.clone();
            io.ns(format!("/{read}"), move |socket: SocketRef| {
                let socket_clone = socket.clone();
                socket.on("auth", |Data(data): Data<String>| {
                    setup_private_access(&data, instance, socket_clone)
                });

                let socket_clone = socket.clone();
                instance_clone.lock().unwrap().on_pub_event(move |event| {
                    emitter::to_public_event_emitter(&event, socket_clone.clone())().unwrap()
                });
            });
            // TODO: The created match should be added to some active-state
            MatchCreated {
                player_write: new_match
                    .players
                    .into_iter()
                    .zip(write.into_iter())
                    .collect(),
                read: read.to_string(),
            }
        }
    };
    let router = listener::listen(router, on_create);
    info!("Listening on {}", DEFAULT_URL);
    axum::serve(listener, router).await.unwrap();
}
