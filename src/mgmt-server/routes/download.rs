use std::sync::Arc;

use crate::{clients::AgentApiClient, error::{Error, Result}, link_download::{LinkDownloadManager, LinkDownloadTarget}};

use fctrl::schema::{AgentOutMessage, AgentResponseWithId};
use log::{error, info};
use rocket::{get, response::stream::ByteStream, State};
use tokio_stream::StreamExt;

use super::DownloadResponder;

#[get("/<link_id>")]
pub async fn download(
    agent_client: &State<Arc<AgentApiClient>>,
    link_download_manager: &State<Arc<LinkDownloadManager>>,
    link_id: String,
) -> Result<DownloadResponder<ByteStream![Vec<u8>]>> {
    match link_download_manager.get_link(link_id).await {
        Some(target) => match target {
            LinkDownloadTarget::Savefile { id } => download_savefile(
                agent_client,
                id,
            ).await,
        }
        None => Err(Error::InvalidLink)
    }
}

pub async fn download_savefile(
    agent_client: &State<Arc<AgentApiClient>>,
    id: String,
) -> Result<DownloadResponder<ByteStream![Vec<u8>]>> {
    let (_operation_id, sub) = agent_client.save_get(id.clone()).await?;
    let download_filename = format!("{}.zip", &id);
    // TODO figure out how to properly handle errors
    let s = sub.filter_map(|event| {
        match serde_json::from_str::<AgentResponseWithId>(&event.content) {
            Ok(m) => {
                match m.content {
                    AgentOutMessage::SaveFile(sb) => {
                        if sb.bytes.len() == 0 {
                            info!("get_savefile completed with total multiparts = {:?}", sb.multipart_seqnum);
                            None
                        } else {
                            Some(sb.bytes)
                        }
                    }
                    c => {
                        error!("Expected AgentOutMessage::SaveFile during get_savefile, got something else: {:?}", c);
                        None
                    },
                }
            }
            Err(e) => {
                error!("Error deserialising event content during get_savefile: {:?}", e);
                None
            }
        }
    });

    Ok(DownloadResponder::new(ByteStream::from(s), download_filename))
}
