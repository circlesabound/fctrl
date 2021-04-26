use fctrl::schema::{ServerStatus, mgmt_server_rest::*};
use rocket::{State, http::Status};
use rocket::{get, post};
use rocket_contrib::json::Json;

use crate::clients::AgentApiClient;
use crate::error::Result;

#[get("/control")]
pub async fn status(agent_client: State<'_, AgentApiClient>) -> Result<Json<ServerControlGetResponse>> {
    let ss = agent_client.inner().server_status().await?;
    let mut num_players = 0;
    let game_status = match ss {
        ServerStatus::NotRunning => GameStatus::NotRunning,
        ServerStatus::PreGame => GameStatus::PreGame,
        ServerStatus::InGame { player_count } => {
            num_players = player_count as i32;
            GameStatus::InGame
        },
        ServerStatus::PostGame => GameStatus::PostGame,
    };
    Ok(Json(ServerControlGetResponse {
        game_status,
        player_count: num_players
    }))
}

#[post("/control/start")]
pub async fn start_server() -> Status {
    Status::Accepted
}

#[post("/control/stop")]
pub async fn stop_server() -> Status {
    Status::Accepted
}
