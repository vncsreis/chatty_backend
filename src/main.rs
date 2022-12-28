mod error;
mod message;
mod ws;

use crate::ws::handle_websocket;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use error::ChattyError::ServerError;
use error::Result;
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

async fn get_rooms(State(state): State<Arc<AppState>>) -> Result<Json<Vec<RoomResponse>>> {
    let rooms = match state.rooms.lock() {
        Ok(rooms) => rooms.clone(),
        Err(err) => {
            println!("{:?}", err);
            return Err(ServerError);
        }
    };

    let mut room_vec: Vec<RoomResponse> = vec![];

    for (id, room) in rooms {
        let new_room = RoomResponse {
            id,
            name: room.name.clone(),
            users: room.user_set.clone(),
        };

        room_vec.push(new_room);
    }

    Ok(Json::from(room_vec))
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
) -> Result<Json<Id>> {
    let mut room = RoomState::new();

    room.name = body.name.to_owned();

    let mut rooms = match state.rooms.lock() {
        Ok(rooms) => rooms,
        Err(err) => {
            println!("{:?}", err);
            return Err(ServerError);
        }
    };

    let id = room.id.clone();

    rooms.insert(room.id, room);

    Ok(Json::from(Id { id }))
}

#[derive(Serialize)]
struct RoomResponse {
    id: Uuid,
    name: String,
    users: HashSet<String>,
}

async fn get_room(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RoomResponse>> {
    let rooms = match state.rooms.lock() {
        Ok(rooms) => rooms,
        Err(err) => {
            println!("{:?}", err);
            return Err(ServerError);
        }
    };

    let room = match rooms.get(&id) {
        Some(room) => room,
        None => {
            println!("No room with id {} found", id);
            return Err(ServerError);
        }
    };

    let response = RoomResponse {
        id,
        name: room.name.clone(),
        users: room.user_set.clone(),
    };

    Ok(Json::from(response))
}
