use std::{io::Cursor, sync::Arc};

use fctrl::schema::{mgmt_server_rest::LogStreamPreviousMarker, OperationId};
use log::error;
use rocket::{
    http::{ContentType, Header, Status},
    response::{Responder, Response},
};

use crate::{guards::HostHeader, ws::WebSocketServer};

pub mod auth;
pub mod buildinfo;
pub mod download;
pub mod logs;
pub mod metrics;
pub mod options;
pub mod proxy;
pub mod server;

pub struct LinkDownloadResponder {
    path: String,
}

impl LinkDownloadResponder {
    fn new(
        link_id: String,
    ) -> LinkDownloadResponder {
        let path = format!("/download/{}", link_id);
        LinkDownloadResponder {
            path,
        }
    }
}

impl<'r> Responder<'r, 'static> for LinkDownloadResponder {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .status(Status::Accepted)
            .header(Header::new("Location", self.path))
            .ok()
    }
}

#[derive(Responder)]
pub struct DownloadResponder<T> {
    inner: T,
    content_disposition: ContentDisposition,
}

impl<T> DownloadResponder<T> {
    pub fn new(content: T, download_filename: String) -> DownloadResponder<T> {
        DownloadResponder {
            inner: content,
            content_disposition: ContentDisposition(download_filename),
        }
    }
}

struct ContentDisposition(String);

impl From<ContentDisposition> for Header<'static> {
    fn from(value: ContentDisposition) -> Self {
        Header::new(
            "Content-Disposition", 
            format!("attachment; filename={}", value.0)
        )
    }
}

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
        // Rocket.rs limitations force us to listen to WS connctions on a different port
        // If reverse proxy through Traefik is enabled, we advertise the same port as regular HTTPS traffic (443),
        // and let routing rules forward to the right port inside the container network.
        // Otherwise, advertise the separate port as normal
        let full_uri = match ws.use_wss {
            true => format!("wss://{}{}", host.hostname, path),
            false => format!("ws://{}:{}{}", host.hostname, ws.port, path),
        };
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
