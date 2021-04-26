use fctrl::schema::{mgmt_server_rest::*, ServerStartSaveFile, ServerStatus};
use rocket::{get, post};
use rocket::{http::Status, State};
use rocket_contrib::json::Json;

use crate::clients::AgentApiClient;
use crate::error::Result;

#[get("/server/control")]
pub async fn status(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<ServerControlStatus>> {
    let ss = agent_client.server_status().await?;
    let mut num_players = 0;
    let game_status = match ss {
        ServerStatus::NotRunning => GameStatus::NotRunning,
        ServerStatus::PreGame => GameStatus::PreGame,
        ServerStatus::InGame { player_count } => {
            num_players = player_count as i32;
            GameStatus::InGame
        }
        ServerStatus::PostGame => GameStatus::PostGame,
    };
    Ok(Json(ServerControlStatus {
        game_status,
        player_count: num_players,
    }))
}

#[post("/server/control/start", data = "<savefile>")]
pub async fn start_server(
    agent_client: State<'_, AgentApiClient>,
    savefile: Json<ServerControlStartPostRequest>,
) -> Result<Status> {
    let start_savefile_args = ServerStartSaveFile::Specific(savefile.into_inner().savefile);
    agent_client.server_start(start_savefile_args).await?;
    Ok(Status::Accepted)
}

#[post("/server/control/stop")]
pub async fn stop_server(agent_client: State<'_, AgentApiClient>) -> Result<Status> {
    agent_client.server_stop().await?;
    Ok(Status::Accepted)
}

#[get("/server/savefile")]
pub async fn get_savefiles(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<Vec<SavefileObject>>> {
    let s = agent_client.save_list().await?;
    let ret = s.into_iter().map(|s| SavefileObject {
        name: s.name,
        last_modified: Some(s.last_modified.to_string()),
    }).collect();
    Ok(Json(ret))
}
