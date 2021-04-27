use std::io::Cursor;

use log::error;
use rocket::{
    http::{ContentType, Status},
    response::Responder,
    Response,
};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    AgentCommunicationError,
    AgentDisconnected,
    AgentInternalError(String),
    AgentTimeout,
    BadRequest(String),

    // Generic wrappers around external error types
    Io(std::io::Error),
    Json(serde_json::error::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Self {
        Error::Json(e)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(e)
    }
}

impl<'r> Responder<'r, 'static> for Error {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        let error_obj = ErrorResponse {
            error: format!("{:?}", self),
        };
        let json;
        match serde_json::to_string(&error_obj) {
            Ok(s) => json = s,
            Err(e) => {
                error!("Error serialising error into JSON: {:?}", e);
                json = "{{\"error\": \"error serialising error message!\"}}".to_owned();
            }
        }

        let status = match self {
            Error::AgentCommunicationError | Error::AgentDisconnected | Error::WebSocket(_) => {
                Status::BadGateway
            }
            Error::AgentTimeout => Status::GatewayTimeout,
            Error::AgentInternalError(_) | Error::Io(_) | Error::Json(_) => {
                Status::InternalServerError
            }
            Error::BadRequest(_) => {
                Status::BadRequest
            }
        };

        Response::build()
            .status(status)
            .header(ContentType::JSON)
            .sized_body(json.len(), Cursor::new(json))
            .ok()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ErrorResponse {
    error: String,
}
