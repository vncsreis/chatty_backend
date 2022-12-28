use crate::{message::WsMessage, AppState};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
pub struct RoomState {
    pub id: Uuid,
    pub user_set: HashSet<String>,
    pub tx: broadcast::Sender<WsMessage>,
    pub name: String,
}

impl RoomState {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            user_set: HashSet::new(),
            tx: broadcast::channel(100).0,
            name: String::new(),
        }
    }
}

pub async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

pub async fn handle_socket(ws: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = ws.split();

    let mut tx = None::<broadcast::Sender<WsMessage>>;
    let mut username = String::new();
    let mut id = None::<Uuid>;

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(msg) = message {
            #[derive(Deserialize)]
            struct Connect {
                username: String,
                room_id: Uuid,
            }

            let connect: Connect = match serde_json::from_str(&msg) {
                Ok(connect) => connect,
                Err(_) => {
                    let _ = sender
                        .send(Message::Text(String::from(
                            "Failed to parse connect message",
                        )))
                        .await;
                    break;
                }
            };

            {
                let mut rooms = state.rooms.lock().unwrap();

                let room = match rooms.get_mut(&connect.room_id) {
                    Some(room) => room,
                    None => {
                        break;
                    }
                };

                tx = Some(room.tx.clone());
                id = Some(connect.room_id);

                if !room.user_set.contains(&connect.username) {
                    room.user_set.insert(connect.username.to_owned());
                    username = connect.username.clone();
                }
            }

            if tx.is_some() && !username.is_empty() && username.as_str() != "SERVER" {
                break;
            } else {
                let _ = sender
                    .send(Message::Text(String::from("Username is already taken")))
                    .await;
                return;
            }
        }
    }

    // let tx = tx.unwrap();
    let tx = match tx {
        Some(tx) => tx,
        None => {
            println!("No TX");
            return;
        }
    };

    let mut rx = tx.subscribe();

    let msg = WsMessage {
        id: Uuid::new_v4(),
        sender: String::from("SERVER"),
        text: format!("{} joined", username),
        time: chrono::Utc::now().timestamp_millis(),
    };
    let _ = tx.send(msg);

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let msg = serde_json::to_string(&msg).unwrap();

            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = {
        let tx = tx.clone();

        tokio::spawn(async move {
            while let Some(Ok(Message::Text(msg))) = receiver.next().await {
                let msg: WsMessage = serde_json::from_str(&msg).unwrap();

                let _ = tx.send(msg);
            }
        })
    };

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    let msg = WsMessage {
        id: Uuid::new_v4(),
        sender: String::from("SERVER"),
        text: format!("{} left", username),
        time: chrono::Utc::now().timestamp_millis(),
    };
    let _ = tx.send(msg);
    let mut rooms = state.rooms.lock().unwrap();

    let id = id.unwrap();

    rooms.get_mut(&id).unwrap().user_set.remove(&username);

    if rooms.get_mut(&id).unwrap().user_set.is_empty() {
        rooms.remove(&id);
    }
}
