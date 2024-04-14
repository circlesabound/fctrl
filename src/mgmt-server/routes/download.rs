use std::sync::Arc;

use crate::{clients::AgentApiClient, error::{Error, Result}, link_download::{LinkDownloadManager, LinkDownloadTarget}};

use rocket::{get, response::stream::ByteStream, State};

#[get("/<link_id>")]
pub async fn download(
    agent_client: &State<Arc<AgentApiClient>>,
    link_download_manager: &State<Arc<LinkDownloadManager>>,
    link_id: String,
) -> Result<ByteStream![Vec<u8>]> {
    match link_download_manager.get_link(link_id).await {
        Some(target) => match target {
            LinkDownloadTarget::Savefile { id } => crate::routes::server::get_savefile_real(
                agent_client,
                id,
            ).await,
        }
        None => Err(Error::InvalidLink)
    }
}

