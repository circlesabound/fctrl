use fctrl::schema::mgmt_server_rest::{AuthInfo, AuthInfoDiscord, OAuthTokenResponse, Provider};
use rocket::{get, post, serde::json::Json, State};

use crate::{
    auth::{AuthManager, AuthProvider},
    error::Result,
    guards::HostHeader,
};

#[get("/auth/info")]
pub async fn info(auth: &State<AuthManager>) -> Result<Json<AuthInfo>> {
    let mut auth_info = AuthInfo {
        provider: match auth.provider {
            AuthProvider::None => Provider::None,
            AuthProvider::Discord { .. } => Provider::Discord,
        },
        discord: None,
    };

    if let AuthProvider::Discord { client_id, .. } = &auth.provider {
        let auth_info_discord = AuthInfoDiscord {
            client_id: client_id.clone(),
        };
        auth_info.discord = Some(Box::new(auth_info_discord));
    }

    Ok(Json(auth_info))
}

#[post("/auth/discord/grant?<code>")]
pub async fn discord_grant<'a>(
    host: HostHeader<'a>,
    auth: &State<AuthManager>,
    code: String,
) -> Result<Json<OAuthTokenResponse>> {
    let redirect_uri = format!("http://{}:4200/oauth-redirect", host.hostname.to_string());
    let resp = auth.oauth_grant(code, redirect_uri).await?;

    Ok(Json(resp))
}

#[post("/auth/discord/refresh")]
pub async fn discord_refresh() -> Result<Json<OAuthTokenResponse>> {
    todo!()
}
