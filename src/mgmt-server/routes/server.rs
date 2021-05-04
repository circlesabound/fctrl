use std::{sync::Arc, time::Duration};

use fctrl::schema::{FactorioVersion, ServerStartSaveFile, ServerStatus, mgmt_server_rest::*};
use rocket::{get, post, put};
use rocket::{http::Status, State};
use rocket_contrib::json::Json;

use crate::{clients::AgentApiClient, guards::HostHeader, ws::WebSocketServer};
use crate::{error::Result, routes::WsStreamingResponder};

#[get("/server/control")]
pub async fn status(agent_client: State<'_, AgentApiClient>) -> Result<Json<ServerControlStatus>> {
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

#[get("/server/install")]
pub async fn get_install(agent_client: State<'_, AgentApiClient>) -> Result<Json<ServerInstallGetResponse>> {
    let version = agent_client.version_get().await?;
    Ok(Json(ServerInstallGetResponse {
        version: version.0,
    }))
}

#[post("/server/install", data = "<body>")]
pub async fn upgrade_install<'a>(
    host: HostHeader<'a>,
    agent_client: State<'a, AgentApiClient>,
    ws: State<'a, Arc<WebSocketServer>>,
    body: Json<ServerInstallPostRequest>,
) -> Result<WsStreamingResponder> {
    let body = body.into_inner();
    let (id, sub) = agent_client
        .version_install(FactorioVersion(body.version), body.force_install.unwrap_or(false))
        .await?;

    let resp = WsStreamingResponder::new(Arc::clone(&ws), host, id);

    let ws = Arc::clone(&ws);
    let path = resp.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}

#[get("/server/savefile")]
pub async fn get_savefiles(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<Vec<SavefileObject>>> {
    let s = agent_client.save_list().await?;
    let ret = s
        .into_iter()
        .map(|s| SavefileObject {
            name: s.name,
            last_modified: Some(s.last_modified.to_string()),
        })
        .collect();
    Ok(Json(ret))
}

#[put("/server/savefile/<id>")]
pub async fn create_savefile<'a>(
    host: HostHeader<'a>,
    agent_client: State<'a, AgentApiClient>,
    ws: State<'a, Arc<WebSocketServer>>,
    id: String,
) -> Result<WsStreamingResponder> {
    let (id, sub) = agent_client.save_create(id).await?;

    let resp = WsStreamingResponder::new(Arc::clone(&ws), host, id);

    let ws = Arc::clone(&ws);
    let path = resp.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}
