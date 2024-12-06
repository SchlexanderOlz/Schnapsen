use axum::{self, routing};
use events::{event_logger, SchnapsenDuoEventType};
use futures::{io::ReadToString, StreamExt};
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions,
        QueuePurgeOptions,
    },
    protocol::channel,
    types::FieldTable,
    BasicProperties,
};
use models::{CreateMatch, GameMode, GameServer, MatchCreated, MatchResult, Ranking};
use schnapsen_rs::{PublicEvent, SchnapsenDuo};
use socketioxide::{
    extract::{Data, SocketRef},
    socket::Socket,
    SocketIo,
};
use std::{
    collections::HashMap, hash::{Hash, Hasher}, sync::{Arc, Mutex}
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

mod emitter;
mod events;
mod match_manager;
mod models;
mod performer;
mod translator;

const CREATE_MATCH_QUEUE: &str = "match-created";
const RESULT_MATCH_QUEUE: &str = "match-result";
const CREATE_GAME_QUEUE: &str = "game-created";
const CREATE_MATCH_REQUEST_QUEUE: &str = "match-create-request";
const HEALTH_CHECK_QUEUE: &str = "health-check";

fn notify_match_result(channel: Arc<lapin::Channel>, result: MatchResult) {
    debug!("Notifying match result: {:?}", result);
    let channel = channel.clone();
    tokio::spawn(async move {
        channel
            .basic_publish(
                "",
                RESULT_MATCH_QUEUE,
                BasicPublishOptions::default(),
                &serde_json::to_vec(&result).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    });
}

fn setup_match_result_handler(
    instance: Arc<Mutex<SchnapsenDuo>>,
    channel: Arc<lapin::Channel>,
    created_match: MatchCreated,
    match_manager: Arc<match_manager::WriteMatchManager>,
) {
    let channel = channel.clone();
    instance.lock().unwrap().on_pub_event(move |event| {
        // TODO|POTERROR: Change this to final result
        if let PublicEvent::Result {
            winner,
            points,
            ranked,
        } = event
        {
            let (loser, loser_points) = ranked.iter().find(|(k, _)| **k != winner).unwrap();

            let result = MatchResult {
                match_id: created_match.read.clone(),
                winners: HashMap::from_iter(vec![(winner.clone(), points)]),
                losers: HashMap::from_iter(vec![(loser.clone(), loser_points.clone())]),
                event_log: match_manager.get_event_log(),
                ranking: Ranking {
                    performances: HashMap::from_iter(vec![
                        (winner.clone(), vec!["win".to_string()]),
                        (loser.clone(), vec!["lose".to_string()]),
                    ]),
                },
            };

            notify_match_result(channel.clone(), result);
        }
    });
}

async fn listen_for_match_create(channel: Arc<lapin::Channel>, io: Arc<SocketIo>) {
    info!("Listening for match create requests");
    channel
        .queue_declare(
            CREATE_MATCH_REQUEST_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    channel
        .queue_declare(
            CREATE_MATCH_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    channel
        .queue_declare(
            RESULT_MATCH_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let on_create = move |new_match: CreateMatch| {
        match_manager::WriteMatchManager::create(io.clone(), new_match)
    };

    let public_url = std::env::var("PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");
    let mut consumer = channel
        .basic_consume(
            CREATE_MATCH_REQUEST_QUEUE,
            format!("schnapsen-duo-server@{}", public_url).as_str(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    while let Some(delivery) = consumer.next().await {
        let on_create = on_create.clone();
        let channel = channel.clone();
        let delivery = delivery.expect("error in consumer");
        delivery.ack(BasicAckOptions::default()).await.expect("ack");
        let new_match: CreateMatch = serde_json::from_slice(&delivery.data).unwrap();

        tokio::spawn(async move {
            let match_manager = on_create(new_match.clone());
            let created_match = match_manager.get_meta().clone();
            let instance = match_manager.get_match();

            setup_match_result_handler(
                instance,
                channel.clone(),
                created_match.clone(),
                match_manager,
            );

            debug!("Created match: {:?}", created_match);
            channel
                .basic_publish(
                    "",
                    CREATE_MATCH_QUEUE,
                    BasicPublishOptions::default(),
                    &serde_json::to_vec(&created_match).unwrap(),
                    BasicProperties::default(),
                )
                .await
                .unwrap();
            debug!(
                "Published match to queue({:?}): {:?}",
                CREATE_MATCH_QUEUE, created_match
            );
        });
    }
}

async fn register_server(
    channel: Arc<lapin::Channel>,
) -> Result<String, Box<dyn std::error::Error>> {
    channel
        .queue_declare(
            CREATE_GAME_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let public_url = std::env::var("PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");
    let private_url =
        std::env::var("PRIVATE_ADDR").expect("SCHNAPSEN_DUO_PRIVATE_ADDR must be set");
    let region = std::env::var("REGION").expect("REGION must be set");

    let reply_to = channel
        .queue_declare("", QueueDeclareOptions::default(), FieldTable::default())
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            reply_to.name().as_str(),
            format!("schnapsen-duo-server-uuid-receive@{}", public_url).as_str(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let server_info = GameServer {
        region,
        game: "Schnapsen".to_string(),
        mode: GameMode {
            name: "duo".to_string(),
            player_count: 2,
            computer_lobby: false,
        },
        server_pub: public_url,
        server_priv: private_url,
        token: "token".to_string(),
        ranking_conf: models::RankingConf {
            max_stars: 5000,
            description: "Schnapsen Duo".to_string(),
            performances: vec![
                models::Performance {
                    name: "win".to_string(),
                    weight: 1,
                },
                models::Performance {
                    name: "lose".to_string(),
                    weight: -1,
                },
            ],
        },
    };

    channel
        .basic_publish(
            "",
            CREATE_GAME_QUEUE,
            BasicPublishOptions::default(),
            &serde_json::to_vec(&server_info).unwrap(),
            BasicProperties::default()
                .with_reply_to(reply_to.name().clone())
                .with_correlation_id(uuid::Uuid::new_v4().to_string().into()),
        )
        .await
        .unwrap();

    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.unwrap();
        delivery.ack(BasicAckOptions::default()).await.expect("ack");

        let server_id = std::string::String::from_utf8(delivery.data).unwrap();
        channel
            .queue_purge(reply_to.name().as_str(), QueuePurgeOptions::default())
            .await
            .unwrap();
        return Ok(server_id);
    }
    Err("No server id was received".into())
}

async fn health_check(channel: Arc<lapin::Channel>, id: String) {
    channel
        .basic_publish(
            "",
            HEALTH_CHECK_QUEUE,
            BasicPublishOptions::default(),
            id.as_bytes(),
            BasicProperties::default(),
        )
        .await
        .unwrap();
}

async fn run_health_check(channel: Arc<lapin::Channel>, id: String) {
    channel
        .queue_declare(
            HEALTH_CHECK_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    loop {
        health_check(channel.clone(), id.clone()).await;
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    info!("Starting Schnapsen Duo Server");

    let (layer, io) = socketioxide::SocketIo::new_layer();
    let io = Arc::new(io);

    let amqp_url = std::env::var("AMQP_URL").expect("AMQP_URL must be set");
    let amqp_conn = lapin::Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
        .await
        .unwrap();

    let channel = Arc::new(amqp_conn.create_channel().await.unwrap());

    let uuid = register_server(channel.clone()).await.unwrap();
    info!("Registered server as {:?}", uuid);

    {
        let channel = channel.clone();
        tokio::spawn(listen_for_match_create(channel, io));
    }

    tokio::spawn(run_health_check(channel, uuid));

    let host_url = std::env::var("HOST_ADDR").expect("HOST_ADDR must be set");
    let listener = tokio::net::TcpListener::bind(host_url.as_str())
        .await
        .unwrap();

    let router = axum::Router::new()
        .layer(layer)
        .layer(CorsLayer::new().allow_origin(Any))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .route("/", routing::get(|| async {}));

    info!("Listening on {}", host_url);
    axum::serve(listener, router).await.unwrap();
}
