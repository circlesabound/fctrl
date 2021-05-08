use std::sync::Arc;

use fctrl::schema::OperationId;
use rocket::{
    http::{Header, Status},
    response::{Responder, Response},
};

use crate::{guards::HostHeader, ws::WebSocketServer};

pub mod proxy;
pub mod server;

pub struct WsStreamingResponder {
    pub path: String,
    full_uri: String,
}

impl WsStreamingResponder {
    fn new(
        ws: Arc<WebSocketServer>,
        host: HostHeader,
        operation_id: OperationId,
    ) -> WsStreamingResponder {
        let path = format!("/operation/{}", operation_id.0);
        let full_uri = format!("ws://{}:{}{}", host.0, ws.port, path);
        WsStreamingResponder { path, full_uri }
    }
}

impl<'r> Responder<'r, 'static> for WsStreamingResponder {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .status(Status::Accepted)
            .header(Header::new("Location", self.full_uri))
            .ok()
    }
}
