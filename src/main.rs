mod message;
mod ws;

use crate::ws::handle_websocket;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower_http::cors::CorsLayer;
use uuid::Uuid;
use ws::RoomState;

pub struct AppState {
    pub rooms: Mutex<HashMap<Uuid, RoomState>>,
}

#[tokio::main]
async fn main() {
    let app_state = Arc::new(AppState {
        rooms: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/ws", get(handle_websocket))
        .route("/room", get(get_rooms))
        .route("/room", post(new_room))
        .route("/room/:id", get(get_room))
        .layer(CorsLayer::very_permissive())
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Serving");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn get_rooms(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rooms = state.rooms.lock().unwrap().clone();

    let mut room_vec: Vec<RoomResponse> = vec![];

    for (id, room) in rooms {
        let new_room = RoomResponse {
            id,
            name: room.name.clone(),
            users: room.user_set.clone(),
        };

        room_vec.push(new_room);
    }

    Json(room_vec)
}

#[derive(Deserialize)]
struct NewRoom {
    name: String,
}

#[derive(Serialize)]
struct Id {
    id: Uuid,
}

async fn new_room(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewRoom>,
) -> impl IntoResponse {
    let mut room = RoomState::new();

    room.name = body.name.to_owned();

    let mut rooms = state.rooms.lock().unwrap();

    let id = room.id.clone();

    rooms.insert(room.id, room);

    Json(Id { id })
}

#[derive(Serialize)]
struct RoomResponse {
    id: Uuid,
    name: String,
    users: HashSet<String>,
}

async fn get_room(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let rooms = state.rooms.lock().unwrap();

    let room = rooms.get(&id).unwrap();

    let response = RoomResponse {
        id,
        name: room.name.clone(),
        users: room.user_set.clone(),
    };

    Json(response)
}
