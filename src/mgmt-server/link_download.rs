use std::collections::HashMap;
use chrono::{DateTime, Utc};
use log::info;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct LinkDownloadManager {
    links: RwLock<HashMap<String, (LinkDownloadTarget, DateTime<Utc>)>>,
}

#[derive(Clone, Debug)]
pub enum LinkDownloadTarget {
    Savefile { id: String },
}

impl LinkDownloadManager {
    pub fn new() -> LinkDownloadManager {
        // TODO periodic job to expire out links
        LinkDownloadManager {
            links: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_link(&self, target: LinkDownloadTarget) -> String {
        let mut w_guard = self.links.write().await;
        let link = Uuid::new_v4().as_simple().to_string();
        info!("Generating download link: {} -> {:?}", link, target);
        w_guard.insert(link.clone(), (target, Utc::now()));
        link
    }

    pub async fn get_link(&self, link: String) -> Option<LinkDownloadTarget> {
        let r_guard = self.links.read().await;
        r_guard.get(&link).map(|(target, _dt)| target.clone())
    }
}
