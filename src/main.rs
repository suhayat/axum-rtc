mod room;
mod signaling;
mod config;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router as AxumRouter,
};
use mediasoup::prelude::*;
use mediasoup::worker::{WorkerLogLevel, WorkerSettings};
use mediasoup::worker_manager::WorkerManager;
use serde::Deserialize;
use tower_http::services::ServeDir;
use tracing::info;

use crate::room::{create_rooms, Rooms};
use crate::config::media_codecs;

#[derive(Clone)]
struct AppState {
    rooms: Rooms,
    router: Router,
}

#[derive(Deserialize)]
struct WsQuery {
    room: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    info!("Creating mediasoup worker...");
    let worker_manager = WorkerManager::new();

    let worker = worker_manager
        .create_worker({
            let mut settings = WorkerSettings::default();
            settings.log_level = WorkerLogLevel::Warn;
            settings
        })
        .await
        .expect("Failed to create mediasoup worker");

    info!("Worker created, creating router...");
    let router = worker
        .create_router(RouterOptions::new(media_codecs()))
        .await
        .expect("Failed to create router");

    info!("Router created with RTP capabilities");

    let rooms = create_rooms();
    let state = AppState {
        rooms: rooms.clone(),
        router: router.clone(),
    };

    let app = AxumRouter::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
        .fallback_service(ServeDir::new("frontend/dist").append_index_html_on_directories(true));

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    info!("🚀 Server running at http://{}", addr);
    axum::serve(listener, app).await.expect("Server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let room_id = query.room;
    let peer_id = uuid::Uuid::new_v4().to_string();

    info!("WebSocket upgrade for room={}, peer={}", room_id, peer_id);

    let rooms = state.rooms.clone();
    let router = state.router.clone();

    ws.on_upgrade(move |socket| async move {
        // Add peer to room before starting handler
        {
            let mut rooms_lock = rooms.lock().await;
            let room = rooms_lock
                .entry(room_id.clone())
                .or_insert_with(|| room::Room::new(room_id.clone(), router.clone()));
            room.add_peer(peer_id.clone());
        }

        signaling::handle_ws(socket, room_id, peer_id, rooms, router).await;
    })
}
