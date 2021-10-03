use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use fctrl::schema::mgmt_server_rest::OAuthTokenResponse;
use log::{error, info, warn};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::error::{Error, Result};

pub struct AuthManager {
    pub provider: AuthProvider,
    refresh_tokens: Arc<Mutex<HashMap<String, (String, DateTime<Utc>)>>>, // mapping from real token to refresh token and expiry
    refresh_tokens_sweep_jh: JoinHandle<()>,
}

impl AuthManager {
    pub fn new(provider: AuthProvider) -> Result<AuthManager> {
        let refresh_tokens: Arc<Mutex<HashMap<String, (String, DateTime<Utc>)>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Background task to sweep the refresh token hashmap every so often
        let refresh_tokens_arc = Arc::clone(&refresh_tokens);
        let refresh_tokens_sweep_jh = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::minutes(10).to_std().unwrap()).await;
                let mut mg = refresh_tokens_arc.lock().await;
                let mut keys_to_remove = vec![];

                // First pass, get keys to remove
                for (k, (_v, expiry)) in mg.iter() {
                    if expiry < &Utc::now() {
                        keys_to_remove.push(k.clone());
                    }
                }

                // Second pass, remove entries
                for k in keys_to_remove {
                    mg.remove(&k);
                }
            }
        });

        let mgr = AuthManager {
            provider,
            refresh_tokens,
            refresh_tokens_sweep_jh,
        };

        Ok(mgr)
    }

    pub async fn oauth_grant(
        &self,
        code: String,
        redirect_uri: String,
    ) -> Result<OAuthTokenResponse> {
        if let AuthProvider::Discord {
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
            info!(
                "sending to discord token url: {}",
                serde_json::to_string(&body)?
            );
            let result = client.post(DISCORD_TOKEN_URL).form(&body).send().await;
            match result {
                Ok(resp) => {
                    info!("response ok");
                    let text = resp.text().await?;
                    info!("token response: {:?}", text);
                    let token_response: OAuthTokenFullResponse = serde_json::from_str(&text)?;

                    // Store the refresh token
                    let expiry = Utc::now() + Duration::seconds(token_response.expires_in);
                    let mut mg = self.refresh_tokens.lock().await;
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
                    error!("error with request to discord token url: {:?}", e);
                    Err(e.into())
                }
            }
        } else {
            Err(Error::AuthInvalid)
        }
    }

    pub async fn oauth_refresh(&self, original_token: String) -> Result<OAuthTokenResponse> {
        if let AuthProvider::Discord {
            client_id,
            client_secret,
        } = &self.provider
        {
            // Get the stored refresh token
            let mut mg = self.refresh_tokens.lock().await;
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
}

impl Drop for AuthManager {
    fn drop(&mut self) {
        self.refresh_tokens_sweep_jh.abort();
    }
}

const DISCORD_TOKEN_URL: &'static str = "https://discord.com/api/oauth2/token";

pub enum AuthProvider {
    None,
    Discord {
        client_id: String,
        client_secret: String,
    },
}

#[derive(serde::Deserialize)]
pub struct OAuthTokenFullResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}
