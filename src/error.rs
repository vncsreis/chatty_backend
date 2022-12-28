use axum::{http::StatusCode, response::IntoResponse};

pub enum ChattyError {
    ServerError,
}

impl IntoResponse for ChattyError {
    fn into_response(self) -> axum::response::Response {
        let message = format!("Internal server error");

        (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
    }
}

pub type Result<T, E = ChattyError> = std::result::Result<T, E>;
