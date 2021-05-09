use std::path::PathBuf;

use rocket::{Responder, http::Header, options};

#[derive(Responder)]
#[response(status = 204)]
pub struct OptionsResponder {
    inner: (),
    allow_header: Header<'static>,
}

impl OptionsResponder {
    pub fn new() -> OptionsResponder {
        OptionsResponder {
            inner: (),
            allow_header: Header::new("Allow", "GET, OPTIONS, POST, PUT"),
        }
    }
}

#[options("/<_any..>")]
pub async fn options(_any: PathBuf) -> OptionsResponder {
    OptionsResponder::new()
}
