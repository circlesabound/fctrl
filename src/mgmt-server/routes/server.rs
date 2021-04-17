use rocket::get;

#[get("/status")]
pub async fn status() -> String {
    "todo".to_owned()
}
