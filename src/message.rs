use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone)]
pub struct WsMessage {
    pub id: Uuid,
    pub sender: String,
    pub text: String,
    pub time: i64,
}

impl WsMessage {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            sender: String::new(),
            text: String::new(),
            time: chrono::Utc::now().timestamp_millis(),
        }
    }
}
