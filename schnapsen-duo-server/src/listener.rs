use std::{collections::HashMap, future::Future, sync::Arc};

use axum::{extract::State, response::Json as JsonResponse, routing::post, Json};
use serde::{Deserialize, Serialize};


#[derive(Deserialize, Debug)]
pub struct CreateMatch {
    pub game: String,
    pub players: Vec<String>,
    pub mode: GameMode,
}

#[derive(Deserialize, Debug)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

#[derive(Serialize, Debug)]
pub struct MatchCreated {
    pub player_write: HashMap<String, String>,
    pub read: String,
}

pub async fn listen<T, F>(router: axum::Router<Arc<T>>, on_create: T) -> axum::Router
where
    T: Send + Sync + 'static + Fn(CreateMatch) -> F,
    F: Send + Sync + 'static + Future<Output = MatchCreated>,
{
   router 
        .route("/", post(handle_create))
        .with_state(Arc::new(on_create))
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
