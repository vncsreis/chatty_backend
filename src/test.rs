//! Example chat application.
//!
//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-chat
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

// Our shared state
struct AppState {
    /// Keys are the name of the channel
    rooms: Mutex<HashMap<String, RoomState>>,
}

struct RoomState {
    /// Previously stored in AppState
    user_set: HashSet<String>,
    /// Previously created in main.
    tx: broadcast::Sender<String>,
}

impl RoomState {
    fn new() -> Self {
        Self {
            // Track usernames per room rather than globally.
            user_set: HashSet::new(),
            // Create a new channel for every room
            tx: broadcast::channel(100).0,
        }
    }
}

#[tokio::main]
async fn main() {
    let app_state = Arc::new(AppState {
        rooms: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/websocket", get(websocket_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // Username gets set in the receive loop, if it's valid.

    // We have more state now that needs to be pulled out of the connect loop
    let mut tx = None::<broadcast::Sender<String>>;
    let mut username = String::new();
    let mut channel = String::new();

    // Loop until a text message is found.
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            #[derive(Deserialize)]
            struct Connect {
                username: String,
                channel: String,
            }

            let connect: Connect = match serde_json::from_str(&name) {
                Ok(connect) => connect,
                Err(error) => {
                    let _ = sender
                        .send(Message::Text(String::from(
                            "Failed to parse connect message",
                        )))
                        .await;
                    break;
                }
            };

            // Scope to drop the mutex guard before the next await
            {
                // If username that is sent by client is not taken, fill username string.
                let mut rooms = state.rooms.lock().unwrap();

                channel = connect.channel.clone();
                let room = rooms.entry(connect.channel).or_insert_with(RoomState::new);

                tx = Some(room.tx.clone());

                if !room.user_set.contains(&connect.username) {
                    room.user_set.insert(connect.username.to_owned());
                    username = connect.username.clone();
                }
            }

            // If not empty we want to quit the loop else we want to quit function.
            if tx.is_some() && !username.is_empty() {
                break;
            } else {
                // Only send our client that username is taken.
                let _ = sender
                    .send(Message::Text(String::from("Username already taken.")))
                    .await;

                return;
            }
        }
    }

    // We know if the loop exited `tx` is not `None`.
    let tx = tx.unwrap();
    // Subscribe before sending joined message.
    let mut rx = tx.subscribe();

    // Send joined message to all subscribers.
    let msg = format!("{} joined.", username);
    let _ = tx.send(msg);

    // This task will receive broadcast messages and send text message to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // We need to access the `tx` variable directly again, so we can't shadow it here.
    // I moved the task spawning into a new block so the original `tx` is still visible later.
    let mut recv_task = {
        // Clone things we want to pass to the receiving task.
        let tx = tx.clone();
        let name = username.clone();

        // This task will receive messages from client and send them to broadcast subscribers.
        tokio::spawn(async move {
            while let Some(Ok(Message::Text(text))) = receiver.next().await {
                // Add username before message.
                let _ = tx.send(format!("{}: {}", name, text));
            }
        })
    };

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Send user left message.
    let msg = format!("{} left.", username);
    let _ = tx.send(msg);
    let mut rooms = state.rooms.lock().unwrap();

    // Remove username from map so new clients can take it.
    rooms.get_mut(&channel).unwrap().user_set.remove(&username);

    // TODO: Check if the room is empty now and remove the `RoomState` from the map.
}
