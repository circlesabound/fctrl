use rocket::request::{FromRequest, Outcome};

pub struct HostHeader<'r>(pub &'r str);

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
                    Some(h) => Outcome::Success(HostHeader(h)),
                    None => Outcome::Forward(()),
                }
            }
            None => Outcome::Forward(()),
        }
    }
}
