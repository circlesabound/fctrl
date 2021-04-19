use rocket::{catch, Request};

#[catch(404)]
pub fn not_found(req: &Request) -> String {
    format!("404 not found")
}
