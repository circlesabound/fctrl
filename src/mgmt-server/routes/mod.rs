use std::{io::Cursor, sync::Arc};

use fctrl::schema::{OperationId, mgmt_server_rest::LogStreamPreviousMarker};
use log::error;
use rocket::{http::{ContentType, Header, Status}, response::{Responder, Response}};

use crate::{guards::HostHeader, ws::WebSocketServer};

pub mod logs;
pub mod options;
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
        let full_uri = format!("ws://{}:{}{}", host.hostname, ws.port, path);
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

pub struct WsStreamingResponderWithPreviousMarker {
    base: WsStreamingResponder,
    marker: LogStreamPreviousMarker,
}

impl WsStreamingResponderWithPreviousMarker {
    fn new(
        ws: Arc<WebSocketServer>,
        host: HostHeader,
        operation_id: OperationId,
        previous_marker: LogStreamPreviousMarker,
    ) -> WsStreamingResponderWithPreviousMarker {
        WsStreamingResponderWithPreviousMarker {
            base: WsStreamingResponder::new(ws, host, operation_id),
            marker: previous_marker,
        }
    }
}

impl<'r> Responder<'r, 'static> for WsStreamingResponderWithPreviousMarker {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        let json = match serde_json::to_string(&self.marker) {
            Ok(s) => s,
            Err(e) => {
                error!("Error serialising previous_marker into JSON: {:?}", e);
                "{{\"error\": \"error serialising previous marker\"}}".to_owned()
            }
        };

        Response::build()
            .status(Status::Accepted)
            .header(Header::new("Location", self.base.full_uri))
            .header(ContentType::JSON)
            .sized_body(json.len(), Cursor::new(json))
            .ok()
    }
}
