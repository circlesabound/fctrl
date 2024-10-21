use std::sync::Arc;

use log::error;
use rocket::{get, serde::json::Json, State};

use crate::clients::AgentApiClient;
use crate::error::Result;

#[get("/system/monitor")]
pub async fn monitor(
    agent_client: &State<Arc<AgentApiClient>>,
) -> Result<Json<fctrl::schema::mgmt_server_rest::SystemResources>> {
    match agent_client.system_resources().await {
        Ok(s) => Ok(Json(fctrl::schema::mgmt_server_rest::SystemResources {
            cpu_total: s.cpu_total,
            cpus: s.cpus,
            mem_total_bytes: s.mem_total_bytes as i64,
            mem_used_bytes: s.mem_used_bytes as i64,
        })),
        Err(e) => {
            error!("Error retrieving agent build version: {:?}", e);
            Err(e)
        },
    }
}
