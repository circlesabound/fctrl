use fctrl::schema::regex::CONTENT_RANGE_RE;
use log::error;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
};

use crate::auth::{AuthnManager, AuthnProvider, AuthorizedUser, AuthzManager, UserIdentity};

pub struct HostHeader<'r> {
    pub hostname: &'r str,
    #[allow(dead_code)]
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

pub struct ContentLengthHeader {
    pub length: usize,
}

#[derive(Debug)]
pub enum ContentLengthHeaderError {
    Missing,
    Format,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ContentLengthHeader {
    type Error = ContentLengthHeaderError;

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Content-Length") {
            Some(h) => {
                if let Ok(length) = h.parse::<usize>() {
                    Outcome::Success(ContentLengthHeader {
                        length,
                    })
                } else {
                    Outcome::Error((Status::BadRequest, ContentLengthHeaderError::Format))
                }
            },
            None => Outcome::Error((Status::BadRequest, ContentLengthHeaderError::Missing)),
        }
    }
}

pub struct ContentRangeHeader {
    pub start: usize,
    #[allow(dead_code)]
    pub end: usize,
    #[allow(dead_code)]
    pub length: usize,
}

#[derive(Debug)]
pub enum ContentRangeHeaderError {
    Missing,
    Format,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ContentRangeHeader {
    type Error = ContentRangeHeaderError;

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Content-Range") {
            Some(h) => {
                if let Some(captures) = CONTENT_RANGE_RE.captures(h) {
                    if let Some(start) = captures.get(1) {
                        if let Ok(start) = start.as_str().parse::<usize>() {
                            if let Some(end) = captures.get(2) {
                                if let Ok(end) = end.as_str().parse::<usize>() {
                                    if let Some(length) = captures.get(3) {
                                        if let Ok(length) = length.as_str().parse::<usize>() {
                                            return Outcome::Success(ContentRangeHeader {
                                                start,
                                                end,
                                                length,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Outcome::Error((Status::BadRequest, ContentRangeHeaderError::Format))
            },
            None => Outcome::Error((Status::BadRequest, ContentRangeHeaderError::Missing)),
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
