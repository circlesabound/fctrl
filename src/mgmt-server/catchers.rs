use rocket::{Request, catch, http::Status, response::NamedFile};

use crate::get_dist_path;

#[catch(404)]
pub fn not_found(_req: &Request) -> String {
    format!("404 not found")
}

#[catch(404)]
pub async fn fallback_to_index_html() -> Option<(Status, NamedFile)> {
    // Required to serve Angular application that uses routing
    NamedFile::open(get_dist_path().join("index.html")).await.ok().map(|nf| (Status::Ok, nf))
}
