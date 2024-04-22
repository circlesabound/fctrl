use std::sync::Arc;

use fctrl::schema::mgmt_server_rest::BuildInfoObject;
use log::error;
use rocket::{get, serde::json::Json, State};

use crate::clients::AgentApiClient;

#[get("/buildinfo")]
pub async fn buildinfo(
    agent_client: &State<Arc<AgentApiClient>>,
) -> Json<BuildInfoObject> {
    let agent_ver = match agent_client.build_version().await {
        Ok(ver) => Some(Box::new(fctrl::schema::mgmt_server_rest::BuildVersion {
            commit_hash: ver.commit_hash,
            timestamp: ver.timestamp,
        })),
        Err(e) => {
            error!("Error retrieving agent build version: {:?}", e);
            None
        },
    };
    Json(BuildInfoObject {
        agent: agent_ver,
        mgmt_server: Some(Box::new(fctrl::schema::mgmt_server_rest::BuildVersion {
            commit_hash: fctrl::util::version::GIT_SHA.unwrap_or("-").to_owned(),
            timestamp: fctrl::util::version::BUILD_TIMESTAMP.to_owned(),
        }))
    })
}
