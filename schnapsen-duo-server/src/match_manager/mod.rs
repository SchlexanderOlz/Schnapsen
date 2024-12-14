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
use serde::Serialize;
use socketioxide::{
    extract::{Data, SocketRef},
    socket::{self, DisconnectReason},
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
    events::{event_logger, EventType, SchnapsenDuoEventType, TimedEvent},
    models::{CreateMatch, MatchAbruptClose, MatchCreated, MatchError, MatchResult, Ranking, Timeout},
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
    on_exit_callbacks: std::sync::Mutex<Vec<Box<dyn FnOnce(Result<MatchResult, MatchAbruptClose>) + Send + Sync>>>,
    min_players: usize,
}

impl WriteMatchManager {
    pub fn create(io: Arc<SocketIo>, new_match: gn_communicator::models::CreateMatch, min_players: usize) -> Arc<Self> {
        debug!("Creating new match: {:?}", new_match);
        let write = new_match.players.clone();

        let io = io.clone();
        let instance = Arc::new(std::sync::Mutex::new(SchnapsenDuo::new(
            new_match.players.as_slice().try_into().unwrap(),
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
            ai_players: new_match.ai_players,
            read: read.to_string(),
            ai: new_match.ai,
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
            on_exit_callbacks: std::sync::Mutex::new(Vec::new()),
            min_players,
        });

        {
            let new = new.clone();
            io.ns(format!("/{read}"), move |socket: SocketRef| {
                new.setup_read_ns(socket)
            });
        }

        new.clone().setup_match_result_handler();
        new.clone().await_initial_connection();

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

    #[inline]
    pub fn get_event_log(&self) -> Vec<TimedEvent<SchnapsenDuoEventType>> {
        self.logger.lock().unwrap().all().into_iter().cloned().collect()
    }

    pub fn on_exit<F>(self: Arc<Self>, callback: F)
    where
        F: FnOnce(Result<MatchResult, MatchAbruptClose>) + Send + Sync + 'static,
    {
        self.on_exit_callbacks.lock().unwrap().push(Box::new(callback));
    }

    fn exit(self: Arc<Self>, reason: Result<MatchResult, MatchError>) {
        if self.exited.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return;
        }

        let reason = reason.map_err(|err| {
            MatchAbruptClose {
                match_id: self.meta.read.clone(),
                reason: err,
            }
        });

        self.exited.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        for callback in self.on_exit_callbacks.lock().unwrap().drain(..) {
            callback(reason.clone());
        }
    }

    fn await_initial_connection(self: Arc<Self>) {
        for player in self.meta.player_write.keys() {
            let (tx, rx) = watch::channel(false);
            self.awaiting_reconnection.lock().unwrap().insert(player.clone(), tx);
            tokio::spawn(self.clone().await_timeout(rx, player.clone()));
        }
    }

    fn setup_match_result_handler(self: Arc<Self>) {
        self.clone().instance.lock().unwrap().on_pub_event(move |event| {
            // TODO|POTERROR: Change this to final result
            if let PublicEvent::Result {
                winner,
                points,
                ranked,
            } = event
            {
                let (loser, loser_points) = ranked.iter().find(|(k, _)| **k != winner).unwrap();

                let result = MatchResult {
                    match_id: self.meta.read.clone(),
                    winners: HashMap::from_iter(vec![(winner.clone(), points)]),
                    losers: HashMap::from_iter(vec![(loser.clone(), loser_points.clone())]),
                    event_log: self.get_event_log(),
                    ranking: Ranking {
                        performances: HashMap::from_iter(vec![
                            (winner.clone(), vec!["win".to_string()]),
                            (loser.clone(), vec!["lose".to_string()]),
                        ]),
                    },
                };

                self.clone().exit(Ok(result));
            }
        });

    }

    async fn play_card_or_timeout(self: Arc<Self>, event: PrivateEvent, player_id: String) {
        if let PrivateEvent::AllowPlayCard = event {
            let (tx, rx) = watch::channel(false);
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
                self.clone().timeout_player(player_id.clone());

                if !self.started.load(std::sync::atomic::Ordering::SeqCst) {
                    self.exit(Err(MatchError::PlayerDidNotJoin(player_id)));
                    return;
                }

                if self.write_connected.read().unwrap().len() < self.min_players {
                    self.exit(Err(MatchError::AllPlayersDisconnected));
                }
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
            for socket in sockets {
                let timeout = timeout.clone();
                tokio::task::spawn(async move {
                    let lock = socket.lock().await;
                    lock
                        .emit("timeout", timeout.clone())
                        .unwrap();
                });
            }
        }
        self.write_connected.write().unwrap().remove(&player_id);
    }

    fn setup_event_log(
        instance: Arc<std::sync::Mutex<SchnapsenDuo>>,
        new_match: &gn_communicator::models::CreateMatch,
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
        write: &str,
        socket: Arc<tokio::sync::Mutex<SocketRef>>,
    ) {
        let player_id = self.meta.player_write.iter().find_map(|(k, v)| {
            if v == write {
                Some(k)
            } else {
                None
            }
        }).cloned();

        if player_id.is_none() {
            return;
        }

        let player_id = player_id.as_ref().unwrap();

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
                    if let Err(err) = emitter::to_private_event_emitter(
                        &event.into() as &TimedEvent<PrivateEvent>
                    )(socket_clone.lock().await.clone())
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

                let should_exit = 'exit: {
                    let mut lock = self.write_connected.write().unwrap();
                    if let Some(sockets) = lock.get_mut(&player_id) {
                        sockets.retain(|socket| {
                            tokio::task::block_in_place(|| socket.blocking_lock().id != disconnected.id)
                        });
                        break 'exit sockets.len() == 0;
                    }
                    true
                };

                if should_exit {
                    tokio::spawn(self.reconnect_or_timeout(player_id));
                }
            },
        );
    }

    async fn reconnect_or_timeout(self: Arc<Self>, player_id: String) {
        let (tx, mut rx) = watch::channel(false);

        self.awaiting_reconnection
            .lock()
            .unwrap()
            .insert(player_id.clone(), tx);

            self.await_timeout(rx, player_id).await;
    }

    async fn handle_auth(
        self: Arc<Self>,
        data: String,
        socket: Arc<tokio::sync::Mutex<SocketRef>>,
    ) {
        debug!("Authenticating: {:?} at Game: {:?}", data, self.match_id);

        self.clone()
            .setup_private_access(&data.clone(), socket.clone())
            .await;
        debug!("Authenticated: {:?} at Game: {:?}", data, self.match_id);

        self.instance.lock().unwrap().on_pub_event(move |event| {
            let socket_clone = socket.clone();
            tokio::task::spawn(async move {
                emitter::to_public_event_emitter(&event.into() as &TimedEvent<PublicEvent>)(
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
    }

    async fn setup_sync_event(self: Arc<Self>, socket: Arc<tokio::sync::Mutex<SocketRef>>) {
        let socket_clone = socket.clone();
        socket
            .lock()
            .await
            .on("sync", move |Data(timestamp): Data<u64>| async move {
                let events: Vec<_> = self
                    .logger
                    .lock()
                    .unwrap()
                    .events_since(timestamp)
                    .into_iter()
                    .cloned()
                    .collect();
                for timed_event in events {
                    emitter::to_private_event_emitter(&timed_event)(
                        socket_clone.lock().await.clone(),
                    ).unwrap();
                }
            });
    }

    async fn listen_for_access_events(self: Arc<Self>, socket: SocketRef) {
        let socket_ptr = Arc::new(tokio::sync::Mutex::new(socket.clone()));
        socket.join(PUBLIC_EVENT_ROOM).unwrap();

        {
            let matchmanager = self.clone();
            let socket_clone = socket_ptr.clone();
            socket.on("auth", move |Data(data): Data<String>| {
                matchmanager.handle_auth(data, socket_clone)
            });
        }

        self.setup_sync_event(socket_ptr).await;
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

        self.listen_for_access_events(socket).await;
    }
}
