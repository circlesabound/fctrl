//! Routes to proxy calls to Factorio Mod Portal API
//! Necessary as mods.factorio.com/api does not implement CORS

use crate::error::Result;

use rocket::{get, response::status};

#[get("/api/mods?<namelist>&<page_size>&<page>")]
pub async fn mod_portal_batch_get(
    namelist: Vec<String>,
    page_size: Option<u32>,
    page: Option<u32>,
) -> Result<String> {
    // rebuild query string
    let mut query_strings_split = vec![];
    query_strings_split.push(
        namelist
            .into_iter()
            .map(|name| format!("namelist={}", name))
            .collect::<Vec<_>>()
            .join("&"),
    );
    if let Some(page_size) = page_size {
        query_strings_split.push(format!("page_size={}", page_size));
    }
    if let Some(page) = page {
        query_strings_split.push(format!("page={}", page));
    }

    let query_string = query_strings_split.join("&");
    let url = format!("https://mods.factorio.com/api/mods?{}", query_string);
    let resp = reqwest::get(url).await?;
    let text = resp.text().await?;
    Ok(text)
}

#[get("/api/mods/<mod_name>")]
pub async fn mod_portal_short_get(
    mod_name: String,
) -> Result<std::result::Result<String, status::NotFound<String>>> {
    let url = format!("https://mods.factorio.com/api/mods/{}", mod_name);
    let resp = reqwest::get(url).await?;
    match resp.error_for_status() {
        Ok(r) => Ok(Ok(r.text().await?)),
        Err(e) => {
            if let Some(reqwest::StatusCode::NOT_FOUND) = e.status() {
                Ok(Err(status::NotFound("Mod not found".to_owned())))
            } else {
                Err(e.into())
            }
        }
    }
}

#[get("/api/mods/<mod_name>/full")]
pub async fn mod_portal_full_get(
    mod_name: String,
) -> Result<std::result::Result<String, status::NotFound<String>>> {
    let url = format!("https://mods.factorio.com/api/mods/{}/full", mod_name);
    let resp = reqwest::get(url).await?;
    match resp.error_for_status() {
        Ok(r) => Ok(Ok(r.text().await?)),
        Err(e) => {
            if let Some(reqwest::StatusCode::NOT_FOUND) = e.status() {
                Ok(Err(status::NotFound("Mod not found".to_owned())))
            } else {
                Err(e.into())
            }
        }
    }
}
