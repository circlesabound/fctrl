use log::error;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
};

use crate::auth::{AuthnManager, AuthnProvider, AuthorizedUser, AuthzManager, UserIdentity};

pub struct HostHeader<'r> {
    pub hostname: &'r str,
    pub host: &'r str,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for HostHeader<'r> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        match request.headers().get_one("Host") {
            Some(h) => {
                // remove the port if any
                match h.split(':').next() {
                    Some(hostname) => Outcome::Success(HostHeader { hostname, host: h }),
                    None => Outcome::Forward(Status::InternalServerError),
                }
            }
            None => Outcome::Forward(Status::InternalServerError),
        }
    }
}

#[derive(Debug)]
pub enum AuthError {
    Missing,
    Malformed,
    TokenInvalid,
    InternalError,
    Unauthorized,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdentity {
    type Error = AuthError;

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        if let Some(authn_mgr) = request.rocket().state::<AuthnManager>() {
            if let AuthnProvider::None = authn_mgr.provider {
                Outcome::Success(UserIdentity::anonymous())
            } else {
                if let Some(h) = request.headers().get_one("Authorization") {
                    if let Some(token) = h.strip_prefix("Bearer ") {
                        if let Ok(id) = authn_mgr.get_id_details(token).await {
                            Outcome::Success(id)
                        } else {
                            Outcome::Error((Status::Forbidden, AuthError::TokenInvalid))
                        }
                    } else {
                        Outcome::Error((Status::Forbidden, AuthError::Malformed))
                    }
                } else {
                    Outcome::Error((Status::Forbidden, AuthError::Missing))
                }
            }
        } else {
            error!("Failed to retrieve AuthnManager, this should never happen!");
            Outcome::Error((Status::InternalServerError, AuthError::InternalError))
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthorizedUser {
    type Error = AuthError;

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        match request.guard::<UserIdentity>().await {
            Outcome::Success(id) => {
                if let Some(authz_mgr) = request.rocket().state::<AuthzManager>() {
                    if authz_mgr.authorize(&id) {
                        Outcome::Success(AuthorizedUser(id))
                    } else {
                        Outcome::Error((Status::Forbidden, AuthError::Unauthorized))
                    }
                } else {
                    error!("Failed to retrieve AuthzManager, this should never happen!");
                    Outcome::Error((Status::InternalServerError, AuthError::InternalError))
                }
            }
            Outcome::Error(f) => Outcome::Error(f),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}
