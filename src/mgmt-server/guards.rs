use log::error;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
};

use crate::auth::{AuthnManager, AuthnProvider, UserIdentity};

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
                    None => Outcome::Forward(()),
                }
            }
            None => Outcome::Forward(()),
        }
    }
}

#[derive(Debug)]
pub enum AuthorizationError {
    Missing,
    Malformed,
    TokenInvalid,
    InternalError,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdentity {
    type Error = AuthorizationError;

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
                            Outcome::Failure((Status::Forbidden, AuthorizationError::TokenInvalid))
                        }
                    } else {
                        Outcome::Failure((Status::Forbidden, AuthorizationError::Malformed))
                    }
                } else {
                    Outcome::Failure((Status::Forbidden, AuthorizationError::Missing))
                }
            }
        } else {
            error!("Failed to retrieve AuthnManager, this should never happen!");
            Outcome::Failure((
                Status::InternalServerError,
                AuthorizationError::InternalError,
            ))
        }
    }
}
