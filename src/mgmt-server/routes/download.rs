use std::sync::Arc;

use crate::{clients::AgentApiClient, error::{Error, Result}, link_download::{LinkDownloadManager, LinkDownloadTarget}};

use fctrl::schema::{AgentOutMessage, AgentResponseWithId};
use futures::{stream, Stream};
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
        Some(target) => {
            let source_stream;
            let download_filename;
            match target {
                LinkDownloadTarget::Savefile { id } => {
                    download_filename = format!("{}.zip", &id);
                    source_stream = download_save(agent_client, id).await?;

                }
                LinkDownloadTarget::ModSettingsDat => {
                    download_filename = "mod-settings.dat".to_owned();
                    source_stream = download_mod_settings_dat(agent_client).await?;
                }
            }

            Ok(DownloadResponder::new(ByteStream::from(source_stream), download_filename))
        }
        None => Err(Error::InvalidLink)
    }
}

async fn download_save(
    agent_client: &State<Arc<AgentApiClient>>,
    id: String,
) -> Result<Box<dyn Stream<Item = Vec<u8>> + Unpin + Send>> {
    let (_operation_id, sub) = agent_client.save_get(id.clone()).await?;
    // TODO figure out how to properly handle errors
    let s = sub.filter_map(|event| {
        match serde_json::from_str::<AgentResponseWithId>(&event.content) {
            Ok(m) => {
                match m.content {
                    AgentOutMessage::SaveFile(sb) => {
                        if sb.is_sentinel() {
                            info!("get_savefile completed with total multipart length = {:?}", sb.multipart_start);
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

    Ok(Box::new(s))
}

async fn download_mod_settings_dat(
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Box<dyn Stream<Item = Vec<u8>> + Unpin + Send>> {
    let bytes = agent_client.mod_settings_get().await?;
    Ok(Box::new(Box::pin(stream::once(async { bytes.bytes }))))
}
