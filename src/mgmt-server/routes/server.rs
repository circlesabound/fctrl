use fctrl::schema::mgmt_server_rest::*;
use rocket::http::Status;
use rocket::{get, post};
use rocket_contrib::json::Json;

#[get("/control")]
pub async fn status() -> Json<ServerControlGetResponse> {
    todo!()
}

#[post("/control/start")]
pub async fn start_server() -> Status {
    Status::Accepted
}

#[post("/control/stop")]
pub async fn stop_server() -> Status {
    Status::Accepted
}
