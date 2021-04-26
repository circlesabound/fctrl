use rocket::{http::Status, response::Responder};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    AgentCommunicationError,
    AgentDisconnected,
    AgentTimeout,

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
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        match self {
            Error::AgentCommunicationError |
            Error::AgentDisconnected |
            Error::WebSocket(_) => {
                Err(Status::BadGateway)
            }
            Error::AgentTimeout => {
                Err(Status::GatewayTimeout)
            }
            Error::Io(_) |
            Error::Json(_) => {
                Err(Status::InternalServerError)
            }
        }
    }
}
