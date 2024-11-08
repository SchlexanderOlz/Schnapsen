use axum::{self, routing};
use listener::MatchCreated;
use schnapsen_rs::SchnapsenDuo;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use std::{
    future,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

mod emitter;
mod listener;
mod performer;
mod translator;

const PUBLIC_EVENT_ROOM: &str = "public-events";

fn setup_private_access(
    player_id: &str,
    instance: Arc<Mutex<SchnapsenDuo>>,
    socket: Arc<tokio::sync::Mutex<SocketRef>>,
) {
    let player = instance.lock().unwrap().get_player(player_id).unwrap();

    debug!("Got player: {:?}", player.borrow().id);

    let socket_clone = socket.clone();
    instance
        .lock()
        .unwrap()
        .on_priv_event(player.clone(), move |event| {
            let socket_clone = socket_clone.clone();
            tokio::task::spawn(async move {
                emitter::to_private_event_emitter(&event)(socket_clone.lock().await.clone())
                    .unwrap();
            });
        });

    let player_id_clone = player_id.to_string();
    tokio::task::spawn(async move {
        let translator = translator::SchnapsenDuoTranslator::listen(socket.clone()).await;
        translator.on_event(move |action| {
            let performer = performer::Performer::new(player_id_clone.clone(), instance.clone());
            let res = performer.perform(action);
            if res.is_err() {
                let socket = socket.clone();
                tokio::task::spawn(async move {
                    socket
                        .lock()
                        .await
                        .emit("error", res.unwrap_err().to_string())
                        .unwrap();
                });
            }
        });
    });
}

fn setup_new_match(
    io: Arc<SocketIo>,
    new_match: listener::CreateMatch,
) -> impl future::Future<Output = MatchCreated> {
    debug!("Creating new match: {:?}", new_match);
    let write = new_match.players.clone();
    let write_len = write.len();

    let io = io.clone();
    let instance = Arc::new(Mutex::new(SchnapsenDuo::new(
        write.as_slice().try_into().unwrap(),
    )));

    let players_connected = Arc::new(Mutex::new(Vec::new()));

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    instance.lock().unwrap().hash(&mut hasher);
    let read = hasher.finish();

    let public_url = std::env::var("PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");
    let private_url =
        std::env::var("PRIVATE_ADDR").expect("SCHNAPSEN_DUO_PRIVATE_ADDR must be set");

    async move {
        io.ns(format!("/{read}"), move |socket: SocketRef| {
            setup_read_ns(socket, instance, read, players_connected, write_len);
        });
        // TODO: The created match should be added to some active-state
        MatchCreated {
            player_write: new_match
                .players
                .into_iter()
                .zip(write.into_iter())
                .collect(),
            read: read.to_string(),
            url_pub: public_url,
            url_priv: private_url,
        }
    }
}

fn setup_read_ns(
    socket: SocketRef,
    instance: Arc<Mutex<SchnapsenDuo>>,
    read: u64,
    connected: Arc<Mutex<Vec<String>>>,
    write_len: usize,
) {
    debug!("New connection to {:?}", read);
    let socket_clone = Arc::new(tokio::sync::Mutex::new(socket.clone()));

    socket.join(PUBLIC_EVENT_ROOM).unwrap();
    socket.on("auth", move |Data(data): Data<String>| {
        debug!("Authenticating: {:?} at Game: {:?}", data, read);

        tokio::task::spawn_blocking(move || {
            setup_private_access(&data.clone(), instance.clone(), socket_clone.clone());
            debug!("Authenticated: {:?} at Game: {:?}", data, read);

            let mut lock = instance.lock().unwrap();
            lock.on_pub_event(move |event| {
                let socket_clone = socket_clone.clone();
                tokio::task::spawn(async move {
                    emitter::to_public_event_emitter(&event)(
                        socket_clone.lock().await.to(PUBLIC_EVENT_ROOM),
                    )
                    .unwrap();
                });
            });
            connected.lock().unwrap().push(data.clone());
            if connected.lock().unwrap().len() == write_len {
                debug!("Starting game: {:?}", read);
                let active_player = lock.get_player(&data);
                lock.set_active_player(active_player.unwrap()).unwrap();
                lock.distribute_cards().unwrap();
            }
        });
    });
}

#[tokio::main]
async fn main() {
    let host_url = std::env::var("HOST_ADDR").expect("HOST_ADDR must be set");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    info!("Starting Schnapsen Duo Server");
    let listener = tokio::net::TcpListener::bind(host_url.as_str())
        .await
        .unwrap();
    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let router = axum::Router::new().layer(layer);

    let on_create = move |new_match: listener::CreateMatch| setup_new_match(io.clone(), new_match);

    let router = listener::listen(router, on_create)
        .layer(CorsLayer::new().allow_origin(Any))
        .route("/", routing::get(|| async {}));
    info!("Listening on {}", host_url);
    axum::serve(listener, router).await.unwrap();
}
