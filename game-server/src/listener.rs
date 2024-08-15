use std::{collections::HashMap, future::Future, net::TcpListener, sync::Arc};

use axum::{
    extract::State, response::IntoResponseParts, response::Json as JsonResponse, routing::post,
    Json,
};
use serde::{Deserialize, Serialize};

const DEFAULT_URL: &str = "0.0.0.0:5000";

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateMatch {
    pub game: String,
    pub players: Vec<String>,
    pub mode: GameMode,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

#[derive(Serialize, Debug)]
pub struct MatchCreated {
    pub player_write: HashMap<String, String>,
    pub read: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct ModeServerMatchCreated {
    pub player_write: HashMap<String, String>,
    pub read: String,
}

pub async fn listen<T, F>(on_create: T)
where
    T: Send + Sync + 'static + Fn(CreateMatch) -> F,
    F: Send + Sync + 'static + Future<Output = MatchCreated>,
{
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();

    let app = axum::Router::new()
        .route("/", post(handle_create))
        .with_state(Arc::new(on_create));

    axum::serve(listener, app).await.unwrap();
}

async fn handle_create<F>(
    State(state): State<Arc<impl Fn(CreateMatch) -> F + Send + Sync + 'static>>,
    Json(payload): Json<CreateMatch>,
) -> JsonResponse<MatchCreated>
where
    F: Future<Output = MatchCreated> + Send + Sync + 'static,
{
    JsonResponse(state(payload).await)
}
