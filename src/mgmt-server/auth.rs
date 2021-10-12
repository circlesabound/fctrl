use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use fctrl::schema::mgmt_server_rest::OAuthTokenResponse;
use log::{error, info, warn};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::error::{Error, Result};

pub struct AuthnManager {
    pub provider: AuthnProvider,
    /// Mapping from access token to identity, cached to avoid continuous external calls to identity provider
    token_to_id_map: Arc<Mutex<HashMap<String, UserIdentity>>>,
    /// Mapping from access token to refresh token and expiry
    refresh_token_map: Arc<Mutex<HashMap<String, (String, DateTime<Utc>)>>>,
    refresh_token_sweep_jh: JoinHandle<()>,
}

impl AuthnManager {
    pub fn new(provider: AuthnProvider) -> Result<AuthnManager> {
        let token_to_id_map = Arc::new(Mutex::new(HashMap::new()));
        let refresh_tokens: Arc<Mutex<HashMap<String, (String, DateTime<Utc>)>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Background task to sweep the token hashmaps every so often
        let token_to_id_map_arc = Arc::clone(&token_to_id_map);
        let refresh_tokens_arc = Arc::clone(&refresh_tokens);
        let refresh_tokens_sweep_jh = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::minutes(10).to_std().unwrap()).await;

                {
                    let mut mg = refresh_tokens_arc.lock().await;
                    let mut tokens_as_keys_to_remove = vec![];

                    // First pass, get keys to remove
                    for (k, (_v, expiry)) in mg.iter() {
                        if *expiry < Utc::now() {
                            tokens_as_keys_to_remove.push(k.clone());
                        }
                    }

                    // Second pass, remove entries
                    let mut id_mg = token_to_id_map_arc.lock().await;
                    for k in tokens_as_keys_to_remove {
                        mg.remove(&k);
                        id_mg.remove(&k);
                    }
                }
            }
        });

        let mgr = AuthnManager {
            provider,
            token_to_id_map,
            refresh_token_map: refresh_tokens,
            refresh_token_sweep_jh: refresh_tokens_sweep_jh,
        };

        Ok(mgr)
    }

    pub async fn oauth_grant(
        &self,
        code: String,
        redirect_uri: String,
    ) -> Result<OAuthTokenResponse> {
        if let AuthnProvider::Discord {
            client_id,
            client_secret,
        } = &self.provider
        {
            let client = reqwest::Client::new();
            let mut body = HashMap::new();
            body.insert("client_id".to_owned(), client_id.clone());
            body.insert("client_secret".to_owned(), client_secret.clone());
            body.insert("grant_type".to_owned(), "authorization_code".to_owned());
            body.insert("code".to_owned(), code);
            body.insert("redirect_uri".to_owned(), redirect_uri);
            let result = client.post(DISCORD_TOKEN_URL).form(&body).send().await;
            match result {
                Ok(resp) => {
                    match resp.error_for_status() {
                        Ok(resp) => {
                            let text = resp.error_for_status()?.text().await?;
                            let token_response =
                                serde_json::from_str::<OAuthTokenFullResponse>(&text)?;

                            // Store the refresh token
                            let expiry = Utc::now() + Duration::seconds(token_response.expires_in);
                            let mut mg = self.refresh_token_map.lock().await;
                            mg.insert(
                                token_response.access_token.clone(),
                                (token_response.refresh_token, expiry),
                            );

                            Ok(OAuthTokenResponse {
                                access_token: token_response.access_token,
                                expires_in: Some(token_response.expires_in as i32),
                            })
                        }
                        Err(e) => {
                            error!(
                                "non-success status response from discord token urL: {:?}",
                                e
                            );
                            Err(e.into())
                        }
                    }
                }
                Err(e) => {
                    error!("error sending request to discord token url: {:?}", e);
                    Err(e.into())
                }
            }
        } else {
            Err(Error::AuthInvalid)
        }
    }

    pub async fn oauth_refresh(&self, original_token: String) -> Result<OAuthTokenResponse> {
        if let AuthnProvider::Discord {
            client_id,
            client_secret,
        } = &self.provider
        {
            // Get the stored refresh token
            let mut mg = self.refresh_token_map.lock().await;
            if let Some((refresh_token, expiry)) = mg.remove(&original_token) {
                if expiry > Utc::now() {
                    // Valid refresh token
                    let client = reqwest::Client::new();
                    let mut body = HashMap::new();
                    body.insert("client_id".to_owned(), client_id.clone());
                    body.insert("client_secret".to_owned(), client_secret.clone());
                    body.insert("grant_type".to_owned(), "refresh_token".to_owned());
                    body.insert("refresh_token".to_owned(), refresh_token);
                    let result = client.post(DISCORD_TOKEN_URL).json(&body).send().await?;
                    let token_response: OAuthTokenFullResponse =
                        serde_json::from_str(&result.text().await?)?;

                    // Store new tokens
                    let expiry = Utc::now() + Duration::seconds(token_response.expires_in);
                    mg.insert(
                        token_response.access_token.clone(),
                        (token_response.refresh_token, expiry),
                    );

                    Ok(OAuthTokenResponse {
                        access_token: token_response.access_token,
                        expires_in: Some(token_response.expires_in as i32),
                    })
                } else {
                    // token present but expired
                    warn!("oauth refresh unavailable as original token is expired");
                    Err(Error::AuthRefreshUnavailable)
                }
            } else {
                Err(Error::BadRequest("invalid token presented".to_owned()))
            }
        } else {
            Err(Error::AuthInvalid)
        }
    }

    pub async fn get_id_details(&self, access_token: impl AsRef<str>) -> Result<UserIdentity> {
        if let AuthnProvider::Discord { .. } = &self.provider {
            // Check if in cache first
            {
                let mut mg = self.token_to_id_map.lock().await;
                if let Some(cached_id) = mg.get(access_token.as_ref()) {
                    return Ok(cached_id.clone());
                }
            }

            let client = reqwest::Client::new();
            let result = client
                .get(DISCORD_IDENTITY_URL)
                .bearer_auth(access_token.as_ref())
                .send()
                .await;
            match result {
                Ok(resp) => {
                    let text = resp.error_for_status()?.text().await?;
                    let discord_user = serde_json::from_str::<DiscordUser>(&text)?;
                    let user_id: UserIdentity = discord_user.into();
                    // Cache result
                    let mut mg = self.token_to_id_map.lock().await;
                    mg.insert(access_token.as_ref().to_string(), user_id.clone());
                    Ok(user_id)
                }
                Err(e) => {
                    error!("error with request to discord identity url: {:?}", e);
                    Err(e.into())
                }
            }
        } else {
            Err(Error::AuthInvalid)
        }
    }
}

impl Drop for AuthnManager {
    fn drop(&mut self) {
        self.refresh_token_sweep_jh.abort();
    }
}

const DISCORD_TOKEN_URL: &'static str = "https://discord.com/api/oauth2/token";
const DISCORD_IDENTITY_URL: &'static str = "https://discord.com/api/users/@me";

pub struct AuthzManager {
    admin: UserIdentity,
}

impl AuthzManager {
    pub fn new(admin: UserIdentity) -> AuthzManager {
        AuthzManager { admin }
    }

    pub fn authorize(&self, id: &UserIdentity) -> bool {
        // TODO
        *id == self.admin
    }
}

/// this scuffed authn system's equivalent of OIDC id_token
#[derive(Clone, PartialEq)]
pub struct UserIdentity {
    pub sub: String,
}

impl UserIdentity {
    pub fn anonymous() -> UserIdentity {
        UserIdentity {
            sub: "anonymous".to_owned(),
        }
    }
}

impl From<DiscordUser> for UserIdentity {
    fn from(du: DiscordUser) -> Self {
        UserIdentity { sub: du.id }
    }
}

pub struct AuthorizedUser(pub UserIdentity);

#[derive(serde::Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    discriminator: String,
}

pub enum AuthnProvider {
    None,
    Discord {
        client_id: String,
        client_secret: String,
    },
}

#[derive(serde::Deserialize)]
struct OAuthTokenFullResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}
