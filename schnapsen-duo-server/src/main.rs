use async_once::AsyncOnce;
use axum::{self, routing};
use futures::future::join_all;
use gn_communicator::{rabbitmq::RabbitMQCommunicator, Communicator};
use lazy_static::lazy_static;
use socketioxide::SocketIo;
use std::sync::Arc;
use tokio::join;
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

lazy_static! {
    static ref communicator: AsyncOnce<RabbitMQCommunicator> = AsyncOnce::new(
        RabbitMQCommunicator::connect(option_env!("AMQP_URL").unwrap())
    );
}

async fn notify_match_close(reason: gn_communicator::models::MatchAbrubtClose) {
    debug!("Notifying match result: {:?}", reason);
    communicator
        .get()
        .await
        .report_match_abrupt_close(&reason)
        .await;
}

async fn notify_match_result(result: gn_communicator::models::MatchResult) {
    debug!("Notifying match result: {:?}", result);
    communicator.get().await.report_match_result(&result).await;
}

fn setup_match_result_handler(match_manager: Arc<match_manager::WriteMatchManager>) {
    match_manager.on_exit(move |event| {
        async_std::task::spawn(async move {
            if let Ok(result) = event {
                notify_match_result(result.into()).await;
            } else if let Err(reason) = event {
                notify_match_close(reason.into()).await;
            }
        });
    });
}

async fn listen_for_match_create(io: Arc<SocketIo>) {
    info!("Listening for match create requests");
    let on_create = move |new_match: gn_communicator::models::CreateMatch| {
        match_manager::WriteMatchManager::create(io.clone(), new_match, 2)
    };

    communicator
        .get()
        .await
        .on_match_create(move |new_match: gn_communicator::models::CreateMatch| {
            let on_create = on_create.clone();
            async move {
                let match_manager = on_create(new_match.clone());
                let created_match = match_manager.get_meta().clone();

                setup_match_result_handler(match_manager);

                communicator
                    .get()
                    .await
                    .report_match_created(&created_match.into())
                    .await;
            }
        })
        .await;
}

async fn register_server(modes: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let public_url = std::env::var("PUBLIC_ADDR").expect("SCHNAPSEN_DUO_PUBLIC_ADDR must be set");
    let private_url =
        std::env::var("PRIVATE_ADDR").expect("SCHNAPSEN_DUO_PRIVATE_ADDR must be set");
    let region = std::env::var("REGION").expect("REGION must be set");

    join_all(modes.into_iter().map(|mode| {
        let server_info = gn_communicator::models::GameServerCreate {
            region: region.clone(),
            game: "Schnapsen".to_string(),
            mode: mode.clone(),
            server_pub: public_url.clone(),
            server_priv: private_url.clone(),
            max_players: 2,
            min_players: 2,
            ranking_conf: gn_communicator::models::RankingConf {
                max_stars: 50,
                description: "Schnapsen Duo".to_string(),
                performances: vec![],
            },
        };

        tokio::spawn(async move {
            communicator
                .get()
                .await
                .create_game(&server_info)
                .await
                .unwrap()
        })
    })).await;
    Ok(())
}

async fn health_check(id: String) {
    communicator.get().await.send_health_check(id).await;
}

async fn run_health_check(id: String) -> ! {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    loop {
        debug!("Waiting for send health-check");
        interval.tick().await;
        health_check(id.clone()).await;
        let server_ids = register_server(&["speed".to_string(), "bummerl".to_string()])
            .await
            .unwrap();
        info!("Registered servers as {:?}", server_ids);
        debug!("Sent health-check");
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

    tokio::spawn(listen_for_match_create(io));

    let private_url =
        std::env::var("PRIVATE_ADDR").expect("SCHNAPSEN_DUO_PRIVATE_ADDR must be set");
    tokio::spawn(run_health_check(private_url));

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
