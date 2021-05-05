use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
    time::Duration,
};

use factorio_mod_settings_parser::ModSettings;
use fctrl::schema::{
    mgmt_server_rest::*, FactorioVersion, ModSettingsBytes, RconConfig, SecretsObject,
    ServerStartSaveFile, ServerStatus,
};
use rocket::{get, post, put, response::content};
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
pub async fn get_install(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<ServerInstallGetResponse>> {
    let version = agent_client.version_get().await?;
    Ok(Json(ServerInstallGetResponse { version: version.0 }))
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
        .version_install(
            FactorioVersion(body.version),
            body.force_install.unwrap_or(false),
        )
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

#[get("/server/config/adminlist")]
pub async fn get_adminlist(agent_client: State<'_, AgentApiClient>) -> Result<Json<Vec<String>>> {
    let al = agent_client.config_adminlist_get().await?;
    Ok(Json(al))
}

#[put("/server/config/adminlist", data = "<body>")]
pub async fn put_adminlist(
    agent_client: State<'_, AgentApiClient>,
    body: Json<Vec<String>>,
) -> Result<()> {
    agent_client.config_adminlist_set(body.into_inner()).await
}

#[get("/server/config/banlist")]
pub async fn get_banlist(agent_client: State<'_, AgentApiClient>) -> Result<Json<Vec<String>>> {
    let al = agent_client.config_banlist_get().await?;
    Ok(Json(al))
}

#[put("/server/config/banlist", data = "<body>")]
pub async fn put_banlist(
    agent_client: State<'_, AgentApiClient>,
    body: Json<Vec<String>>,
) -> Result<()> {
    agent_client.config_banlist_set(body.into_inner()).await
}

#[get("/server/config/whitelist")]
pub async fn get_whitelist(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<ServerConfigWhiteList>> {
    let wl = agent_client.config_whitelist_get().await?;
    let resp = ServerConfigWhiteList {
        enabled: wl.enabled,
        users: wl.users,
    };
    Ok(Json(resp))
}

#[put("/server/config/whitelist", data = "<body>")]
pub async fn put_whitelist(
    agent_client: State<'_, AgentApiClient>,
    body: Json<ServerConfigWhiteList>,
) -> Result<()> {
    let body = body.into_inner();
    agent_client
        .config_whitelist_set(body.enabled, body.users)
        .await
}

#[get("/server/config/rcon")]
pub async fn get_rcon_config(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<ServerConfigRconGetResponse>> {
    let rcon_config = agent_client.config_rcon_get().await?;
    let resp = ServerConfigRconGetResponse {
        port: rcon_config.port as i32,
        password: rcon_config.password,
    };
    Ok(Json(resp))
}

#[put("/server/config/rcon", data = "<body>")]
pub async fn put_rcon_config(
    agent_client: State<'_, AgentApiClient>,
    body: Json<RconConfig>,
) -> Result<()> {
    agent_client.config_rcon_set(body.into_inner()).await
}

#[get("/server/config/secrets")]
pub async fn get_secrets(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<ServerConfigSecrets>> {
    let secrets = agent_client.config_secrets_get().await?;
    let resp = ServerConfigSecrets {
        username: secrets.username,
        token: None,
    };
    Ok(Json(resp))
}

#[put("/server/config/secrets", data = "<body>")]
pub async fn put_secrets(
    agent_client: State<'_, AgentApiClient>,
    body: Json<SecretsObject>,
) -> Result<()> {
    agent_client.config_secrets_set(body.into_inner()).await
}

#[get("/server/config/server-settings")]
pub async fn get_server_settings(
    agent_client: State<'_, AgentApiClient>,
) -> Result<content::Json<String>> {
    let json_str = agent_client.config_server_settings_get().await?;
    Ok(content::Json(json_str))
}

#[put("/server/config/server-settings", data = "<body>")]
pub async fn put_server_settings(
    agent_client: State<'_, AgentApiClient>,
    body: String,
) -> Result<()> {
    agent_client.config_server_settings_set(body).await
}

#[get("/server/mods/list")]
pub async fn get_mods_list(
    agent_client: State<'_, AgentApiClient>,
) -> Result<Json<Vec<ModObject>>> {
    let mod_list = agent_client.mod_list_get().await?;
    // Need to convert into the codegen type
    let resp = mod_list
        .into_iter()
        .map(|mo| ModObject {
            name: mo.name,
            version: mo.version,
        })
        .collect();
    Ok(Json(resp))
}

#[post("/server/mods/list", data = "<body>")]
pub async fn apply_mods_list<'a>(
    host: HostHeader<'a>,
    agent_client: State<'a, AgentApiClient>,
    ws: State<'a, Arc<WebSocketServer>>,
    body: Json<Vec<ModObject>>,
) -> Result<WsStreamingResponder> {
    // Convert from the codegen type
    let mod_list = body
        .into_inner()
        .into_iter()
        .map(|mo| fctrl::schema::ModObject {
            name: mo.name,
            version: mo.version,
        })
        .collect();

    let (id, sub) = agent_client.mod_list_set(mod_list).await?;

    let resp = WsStreamingResponder::new(Arc::clone(&ws), host, id);

    let ws = Arc::clone(&ws);
    let path = resp.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}

#[get("/server/mods/settings")]
pub async fn get_mod_settings(
    agent_client: State<'_, AgentApiClient>,
) -> Result<content::Json<String>> {
    let bytes = agent_client.mod_settings_get().await?;
    let ms = ModSettings::try_from(bytes.0.as_ref())?;
    let json_str = serde_json::to_string(&ms)?;

    Ok(content::Json(json_str))
}

#[put("/server/mods/settings", data = "<body>")]
pub async fn put_mod_settings(agent_client: State<'_, AgentApiClient>, body: String) -> Result<()> {
    let ms: ModSettings = serde_json::from_str(&body)?;
    let bytes = ms.try_into()?;
    agent_client.mod_settings_set(ModSettingsBytes(bytes)).await
}
