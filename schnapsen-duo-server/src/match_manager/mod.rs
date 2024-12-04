use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, AtomicI8},
        Arc, RwLock,
    },
    time::Duration,
};

use futures::{lock::Mutex, task};
use schnapsen_rs::{PrivateEvent, PublicEvent, SchnapsenDuo};
use socketioxide::{
    extract::{Data, SocketRef},
    socket::DisconnectReason,
    SocketIo,
};
use tokio::{
    select,
    sync::watch::{self, Receiver, Sender},
};
use tracing::debug;
use tracing::error;

use crate::{
    emitter,
    events::{event_logger, SchnapsenDuoEventType},
    models::{CreateMatch, MatchCreated, Timeout},
    performer, translator,
};
const PUBLIC_EVENT_ROOM: &str = "public-events";
const FORCE_MOVE_TIMEOUT: u64 = 30;

pub struct WriteMatchManager {
    instance: Arc<std::sync::Mutex<SchnapsenDuo>>,
    meta: MatchCreated,
    match_id: String,
    write_connected: std::sync::RwLock<HashMap<String, Vec<Arc<tokio::sync::Mutex<SocketRef>>>>>,
    awaiting_reconnection: std::sync::Mutex<HashMap<String, Sender<bool>>>,
    logger: Arc<std::sync::Mutex<event_logger::EventLogger<SchnapsenDuoEventType>>>,
    exited: AtomicI8,
    started: AtomicBool,
}

impl WriteMatchManager {
    pub fn create(io: Arc<SocketIo>, new_match: CreateMatch) -> Arc<Self> {
        debug!("Creating new match: {:?}", new_match);
        let write = new_match.players.clone();

        let io = io.clone();
        let instance = Arc::new(std::sync::Mutex::new(SchnapsenDuo::new(
            write.as_slice().try_into().unwrap(),
        )));

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        instance.lock().unwrap().hash(&mut hasher);
        let read = hasher.finish();

        let public_url =
            std::env::var("PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");
        let private_url =
            std::env::var("PRIVATE_ADDR").expect("SCHNAPSEN_DUO_PRIVATE_ADDR must be set");
        let region = std::env::var("REGION").expect("REGION must be set");

        let logger = Self::setup_event_log(instance.clone(), &new_match);

        let meta = MatchCreated {
            region,
            game: new_match.game,
            mode: new_match.mode,
            player_write: new_match
                .players
                .into_iter()
                .zip(write.into_iter())
                .collect(),
            read: read.to_string(),
            url_pub: public_url,
            url_priv: private_url,
        };

        let new = Arc::new(Self {
            instance: instance.clone(),
            meta,
            logger,
            match_id: read.to_string(),
            write_connected: RwLock::new(HashMap::new()),
            exited: AtomicI8::new(0),
            started: AtomicBool::new(false),
            awaiting_reconnection: std::sync::Mutex::new(HashMap::new()),
        });

        {
            let new = new.clone();
            io.ns(format!("/{read}"), move |socket: SocketRef| {
                new.setup_read_ns(socket)
            });
        }

        // TODO: The created match should be added to some active-state
        new
    }

    #[inline]
    pub fn get_meta(&self) -> MatchCreated {
        self.meta.clone()
    }

    #[inline]
    pub fn get_match(&self) -> Arc<std::sync::Mutex<SchnapsenDuo>> {
        self.instance.clone()
    }

    async fn play_card_or_timeout(self: Arc<Self>, event: PrivateEvent, player_id: String) {
        let (tx, rx) = watch::channel(false);
        if let PrivateEvent::AllowPlayCard = event {
            let player_id_copy = player_id.clone();
            let on_play_card = move |event| {
                if let PublicEvent::PlayCard { user_id, card: _ } = event {
                    if player_id_copy == user_id {
                        let _ = tx.send(true);
                    }
                }
            };

            self.instance
                .lock()
                .unwrap()
                .on_pub_event(on_play_card.clone());

            self.clone().await_timeout(rx, player_id).await;
            self.instance.lock().unwrap().off_pub_event(on_play_card);
        }
    }

    async fn await_timeout(self: Arc<Self>, mut rx: Receiver<bool>, player_id: String) {
        select! {
            _ = rx.changed() => {}
            _ = tokio::time::sleep(Duration::from_secs(FORCE_MOVE_TIMEOUT)) => {
                self.exited.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.clone().timeout_player(player_id.clone());
            }
        };
    }

    fn wait_for_move(self: Arc<Self>, player_id: String) {
        let player = self
            .instance
            .lock()
            .unwrap()
            .get_player(&player_id)
            .unwrap();

        self.clone()
            .instance
            .lock()
            .unwrap()
            .on_priv_event(player, move |event| {
                tokio::task::spawn(self.clone().play_card_or_timeout(event, player_id.clone()));
            });
    }

    fn timeout_player(self: Arc<Self>, player_id: String) {
        let timeout = Timeout {
            user_id: player_id.clone(),
            reason: "Has not made move".to_string(), // TODO: Make the reason dynamic
        };

        debug!("Timing out player: {:?}", player_id);
        for (k, sockets) in self.write_connected.read().unwrap().clone().into_iter() {
            debug!("Telling all sockets of player: {:?}", k);
            for socket in sockets {
                let timeout = timeout.clone();
                tokio::task::spawn(async move {
                    socket
                        .lock()
                        .await
                        .emit("timeout", timeout.clone())
                        .unwrap();
                });
            }
        }
    }

    fn setup_event_log(
        instance: Arc<std::sync::Mutex<SchnapsenDuo>>,
        new_match: &CreateMatch,
    ) -> Arc<std::sync::Mutex<event_logger::EventLogger<SchnapsenDuoEventType>>> {
        let logger = Arc::new(std::sync::Mutex::new(event_logger::EventLogger::new()));
        {
            let logger = logger.clone();
            instance.lock().unwrap().on_pub_event(move |event| {
                logger
                    .lock()
                    .unwrap()
                    .log(SchnapsenDuoEventType::Public(event).into());
            });
        }

        for player in &new_match.players {
            let mut instance_lock = instance.lock().unwrap();
            let player = instance_lock.get_player(&player).unwrap();

            let logger = logger.clone();
            instance_lock.on_priv_event(player, move |event| {
                logger
                    .lock()
                    .unwrap()
                    .log(SchnapsenDuoEventType::Private(event).into());
            });
        }

        logger
    }

    async fn setup_private_access(
        self: Arc<Self>,
        player_id: &str,
        socket: Arc<tokio::sync::Mutex<SocketRef>>,
    ) {
        self.write_connected
            .write()
            .unwrap()
            .entry(player_id.to_string())
            .or_insert(Vec::new())
            .push(socket.clone());

        if let Some(rx) = self.awaiting_reconnection.lock().unwrap().remove(player_id) {
            rx.send(true).unwrap();
        }

        let player = self.instance.lock().unwrap().get_player(player_id).unwrap();

        debug!("Got player: {:?}", player.read().unwrap().id);

        let socket_clone = socket.clone();
        self.instance
            .lock()
            .unwrap()
            .on_priv_event(player.clone(), move |event| {
                debug!("Got private event: {:?}", event);
                let socket_clone = socket_clone.clone();
                tokio::task::spawn(async move {
                    if let Err(err) =
                        emitter::to_private_event_emitter(&event)(socket_clone.lock().await.clone())
                    {
                        error!("Error emitting private event: {:?}", err);
                    }
                });
            });

        self.clone().wait_for_move(player_id.to_string());

        let player_id_clone = player_id.to_string();
        let instance = self.instance.clone();

        let disconnect_socket = socket.clone();
        let match_manager = self.clone();

        let translator = translator::SchnapsenDuoTranslator::listen(socket.clone()).await;

        let performer = performer::Performer::new(player_id_clone.clone(), instance.clone());
        translator.on_event(move |action| {
            debug!("Got action: {:?}", action);
            if match_manager
                .exited
                .load(std::sync::atomic::Ordering::SeqCst)
                > 0
            {
                return;
            }

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

        self.handle_disconnect(disconnect_socket, player_id.to_string())
            .await;
    }

    async fn handle_disconnect(
        self: Arc<Self>,
        socket: Arc<tokio::sync::Mutex<SocketRef>>,
        player_id: String,
    ) {
        socket.lock().await.on_disconnect(
            move |disconnected: SocketRef, reason: DisconnectReason| {
                debug!("Player: {:?} disconnected", player_id);
                let match_manager = self.clone();

                let should_exit = {
                    let mut lock = self.write_connected.write().unwrap();
                    let sockets = lock.get_mut(&player_id).unwrap();

                    sockets.retain(|socket| {
                        tokio::task::block_in_place(|| socket.blocking_lock().id != disconnected.id)
                    });
                    sockets.len() == 0
                };

                if should_exit {
                    tokio::spawn(async move {
                        let (tx, mut rx) = watch::channel(false);

                        self.awaiting_reconnection
                            .lock()
                            .unwrap()
                            .insert(player_id.clone(), tx);

                        select! {
                            _ = rx.changed() => {}
                            _ = tokio::time::sleep(Duration::from_secs(FORCE_MOVE_TIMEOUT)) => {
                                match_manager.timeout_player(player_id.clone());
                            }
                        }
                    });
                }
            },
        );
    }

    async fn handle_auth(self: Arc<Self>, socket: SocketRef) {
        let socket_clone = Arc::new(tokio::sync::Mutex::new(socket.clone()));

        socket.join(PUBLIC_EVENT_ROOM).unwrap();
        socket.on("auth", move |Data(data): Data<String>| async move {
            debug!("Authenticating: {:?} at Game: {:?}", data, self.match_id);

            self.clone()
                .setup_private_access(&data.clone(), socket_clone.clone())
                .await;
            debug!("Authenticated: {:?} at Game: {:?}", data, self.match_id);

            self.instance.lock().unwrap().on_pub_event(move |event| {
                let socket_clone = socket_clone.clone();
                tokio::task::spawn(async move {
                    emitter::to_public_event_emitter(&event)(
                        socket_clone.lock().await.to(PUBLIC_EVENT_ROOM),
                    )
                    .unwrap();
                });
            });

            if self.write_connected.read().unwrap().len() == self.meta.player_write.len()
                && !self.started.swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                self.start_match(data);
            };
        });
    }

    fn start_match(self: Arc<Self>, begin_player_id: String) {
        let mut lock = self.instance.lock().unwrap();
        debug!("Starting game: {:?}", self.match_id);
        let active_player = lock.get_player(&begin_player_id);
        lock.set_active_player(active_player.unwrap()).unwrap();
        lock.distribute_cards().unwrap();
    }

    async fn setup_read_ns(self: Arc<Self>, socket: SocketRef) {
        debug!("New connection to {:?}", self.match_id);

        self.handle_auth(socket).await;
    }
}
