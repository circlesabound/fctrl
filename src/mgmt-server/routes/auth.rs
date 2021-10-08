use fctrl::schema::mgmt_server_rest::{AuthInfo, AuthInfoDiscord, OAuthTokenResponse, Provider};
use rocket::{get, post, serde::json::Json, State};

use crate::{auth::{AuthnManager, AuthnProvider, UserIdentity}, error::{Error, Result}, guards::HostHeader};

#[get("/auth/info")]
pub async fn info(auth: &State<AuthnManager>) -> Result<Json<AuthInfo>> {
    let mut auth_info = AuthInfo {
        provider: match auth.provider {
            AuthnProvider::None => Provider::None,
            AuthnProvider::Discord { .. } => Provider::Discord,
        },
        discord: None,
    };

    if let AuthnProvider::Discord { client_id, .. } = &auth.provider {
        let auth_info_discord = AuthInfoDiscord {
            client_id: client_id.clone(),
        };
        auth_info.discord = Some(Box::new(auth_info_discord));
    }

    Ok(Json(auth_info))
}

#[post("/auth/discord/grant?<code>&<redirect_uri>")]
pub async fn discord_grant<'a>(
    host: HostHeader<'a>,
    auth: &State<AuthnManager>,
    code: String,
    redirect_uri: String,
) -> Result<Json<OAuthTokenResponse>> {
    match urlencoding::decode(&redirect_uri) {
        Ok(redirect_uri) => {
            let resp = auth.oauth_grant(code, redirect_uri.to_string()).await?;
            Ok(Json(resp))
        }
        Err(e) => Err(Error::BadRequest("Unable to decode value of redirect_uri parameter".to_owned()))
    }
}

#[post("/auth/discord/refresh")]
pub async fn discord_refresh(
    _identity: UserIdentity,
) -> Result<Json<OAuthTokenResponse>> {
    // TODO
    Err(Error::NotImplemented)
}
