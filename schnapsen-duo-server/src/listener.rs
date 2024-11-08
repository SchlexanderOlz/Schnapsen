use std::{collections::HashMap, future::Future, sync::Arc};

use axum::{extract::State, response::Json as JsonResponse, routing::post, Json};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::models::{CreateMatch, GameMode, MatchCreated};


pub fn listen<T, F>(router: axum::Router<Arc<T>>, on_create: T) -> axum::Router
where
    T: Send + Sync + 'static + Fn(CreateMatch) -> F,
    F: Send + Sync + 'static + Future<Output = MatchCreated>,
{
   router 
        .route("/", post(handle_create))
        .with_state(Arc::new(on_create))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()).into_inner())
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
