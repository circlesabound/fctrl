use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
    time::Duration,
};

use factorio_file_parser::ModSettings;
use fctrl::schema::{
    mgmt_server_rest::*, FactorioVersion, ModSettingsBytes, RconConfig, SaveBytes, SecretsObject, ServerSettingsConfig, ServerStartSaveFile, ServerStatus
};
use rocket::{data::ToByteUnit, delete, serde::json::Json, Data};
use rocket::{get, post, put};
use rocket::{http::Status, State};

use crate::{
    auth::AuthorizedUser, clients::AgentApiClient, guards::{ContentLengthHeader, ContentRangeHeader, HostHeader}, link_download::{LinkDownloadManager, LinkDownloadTarget}, ws::WebSocketServer
};
use crate::{error::Result, routes::WsStreamingResponder};

use super::LinkDownloadResponder;

#[get("/server/control")]
pub async fn status(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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

#[post("/server/control/create", data = "<savefile>")]
pub async fn create_savefile<'a>(
    host: HostHeader<'a>,
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    ws: &State<Arc<WebSocketServer>>,
    savefile: Json<ServerControlCreatePostRequest>,
) -> Result<WsStreamingResponder> {
    let (id, sub) = agent_client.save_create(savefile.into_inner().savefile).await?;

    let resp = WsStreamingResponder::new(Arc::clone(&ws), host, id);

    let ws = Arc::clone(&ws);
    let path = resp.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}

#[post("/server/control/start", data = "<savefile>")]
pub async fn start_server(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    savefile: Json<ServerControlStartPostRequest>,
) -> Result<Status> {
    let start_savefile_args = ServerStartSaveFile::Specific(savefile.into_inner().savefile);
    agent_client.server_start(start_savefile_args).await?;
    Ok(Status::Accepted)
}

#[post("/server/control/stop")]
pub async fn stop_server(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Status> {
    agent_client.server_stop().await?;
    Ok(Status::Accepted)
}

#[get("/server/install")]
pub async fn get_install(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<ServerInstallGetResponse>> {
    let version = agent_client.version_get().await?.map(|v| v.0);
    Ok(Json(ServerInstallGetResponse { version }))
}

#[post("/server/install", data = "<body>")]
pub async fn upgrade_install<'a>(
    host: HostHeader<'a>,
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    ws: &State<Arc<WebSocketServer>>,
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

#[get("/server/savefiles")]
pub async fn get_savefiles(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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

#[delete("/server/savefiles/<id>")]
pub async fn delete_savefile(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    id: String,
) -> Result<()> {
    agent_client.save_delete(id).await
}

#[get("/server/savefiles/<id>")]
pub async fn get_savefile<'a>(
    _a: AuthorizedUser,
    link_download_manager: &State<Arc<LinkDownloadManager>>,
    id: String,
) -> Result<LinkDownloadResponder> {
    let link_id = link_download_manager.create_link(LinkDownloadTarget::Savefile { id }).await;
    Ok(LinkDownloadResponder::new(link_id))
}

#[put("/server/savefiles/<id>", data = "<body>")]
pub async fn put_savefile(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    id: String,
    body: Data<'_>,
    content_length: ContentLengthHeader,
    content_range: ContentRangeHeader,
) -> Result<()> {
    let chunk_stream = body.open(content_length.length.bytes());
    let savebytes = SaveBytes {
        multipart_start: Some(content_range.start),
        bytes: chunk_stream.into_bytes().await?.into_inner(),
    };
    agent_client.save_put(id, savebytes).await?;
    Ok(())
}

#[get("/server/config/adminlist")]
pub async fn get_adminlist(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<Vec<String>>> {
    let al = agent_client.config_adminlist_get().await?;
    Ok(Json(al))
}

#[put("/server/config/adminlist", data = "<body>")]
pub async fn put_adminlist(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<Vec<String>>,
) -> Result<()> {
    agent_client.config_adminlist_set(body.into_inner()).await
}

#[get("/server/config/banlist")]
pub async fn get_banlist(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<Vec<String>>> {
    let al = agent_client.config_banlist_get().await?;
    Ok(Json(al))
}

#[put("/server/config/banlist", data = "<body>")]
pub async fn put_banlist(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<Vec<String>>,
) -> Result<()> {
    agent_client.config_banlist_set(body.into_inner()).await
}

#[get("/server/config/whitelist")]
pub async fn get_whitelist(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<ServerConfigWhiteList>,
) -> Result<()> {
    let body = body.into_inner();
    agent_client
        .config_whitelist_set(body.enabled, body.users)
        .await
}

#[get("/server/config/rcon")]
pub async fn get_rcon_config(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<RconConfig>,
) -> Result<()> {
    agent_client.config_rcon_set(body.into_inner()).await
}

#[get("/server/config/secrets")]
pub async fn get_secrets(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<SecretsObject>,
) -> Result<()> {
    agent_client.config_secrets_set(body.into_inner()).await
}

#[get("/server/config/server-settings")]
pub async fn get_server_settings(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<ServerSettingsConfig>> {
    let json_str = agent_client.config_server_settings_get().await?;
    Ok(Json(json_str))
}

#[put("/server/config/server-settings", data = "<body>")]
pub async fn put_server_settings(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<ServerSettingsConfig>,
) -> Result<()> {
    agent_client.config_server_settings_set(body.into_inner()).await
}

#[get("/server/mods/list")]
pub async fn get_mods_list(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
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
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    ws: &State<Arc<WebSocketServer>>,
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
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<ModSettings>> {
    let ms_bytes = agent_client.mod_settings_get().await?;
    let ms = ModSettings::try_from(ms_bytes.bytes.as_ref())?;

    Ok(Json(ms))
}

#[put("/server/mods/settings", data = "<body>")]
pub async fn put_mod_settings(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: String,
) -> Result<()> {
    let ms: ModSettings = serde_json::from_str(&body)?;
    let bytes = ms.try_into()?;
    agent_client.mod_settings_set(ModSettingsBytes { bytes }).await
}

#[get("/server/mods/settings-dat")]
pub async fn get_mod_settings_dat(
    _a: AuthorizedUser,
    link_download_manager: &State<Arc<LinkDownloadManager>>,
) -> Result<LinkDownloadResponder> {
    let link_id = link_download_manager.create_link(LinkDownloadTarget::ModSettingsDat).await;
    Ok(LinkDownloadResponder::new(link_id))
}

#[put("/server/mods/settings-dat", data = "<body>")]
pub async fn put_mod_settings_dat(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Vec<u8>,
) -> Result<()> {
    agent_client.mod_settings_set(ModSettingsBytes { bytes: body } ).await
}

#[post("/server/rcon", data = "<body>")]
pub async fn send_rcon_command(
    _a: AuthorizedUser,
    agent_client: &State<Arc<AgentApiClient>>,
    body: Json<RconCommandRequest>,
) -> Result<Json<RconCommandResponse>> {
    let command = body.into_inner().command;
    let response = agent_client.rcon_command(command).await?;
    Ok(Json(RconCommandResponse { response }))
}
