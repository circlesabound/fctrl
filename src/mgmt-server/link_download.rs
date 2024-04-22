use std::{collections::HashMap, sync::Arc};
use chrono::{DateTime, Duration, Utc};
use log::info;
use tokio::{select, sync::RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CLEANUP_INTERVAL: Duration = Duration::minutes(15);
const LINK_EXPIRY: Duration = Duration::minutes(60);

type LinkMap = Arc<RwLock<HashMap<String, (LinkDownloadTarget, DateTime<Utc>)>>>;

pub struct LinkDownloadManager {
    links: LinkMap,
    _cleanup_task_ct: CancellationToken,
}

#[derive(Clone, Debug)]
pub enum LinkDownloadTarget {
    Savefile { id: String },
    ModSettingsDat,
}

impl LinkDownloadManager {
    pub async fn new() -> LinkDownloadManager {
        let links = LinkMap::default();
        let links_clone = Arc::clone(&links);
        let cancellation_token = CancellationToken::new();
        let _cleanup_task_ct = cancellation_token.clone();
        tokio::spawn(async move {
            Self::cleanup_job(links_clone, cancellation_token).await;
        });
        LinkDownloadManager {
            links,
            _cleanup_task_ct,
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

    async fn cleanup_job(links: LinkMap, cancellation_token: CancellationToken) {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }
                _ = tokio::time::sleep(CLEANUP_INTERVAL.to_std().unwrap()) => {
                    let mut w_guard = links.write().await;
                    let now = Utc::now();
                    w_guard.retain(|link, (target, dt)| {
                        let should_remove = now - *dt > LINK_EXPIRY;
                        if should_remove {
                            info!("Expiring download link: {} -> {:?}", link, target);
                        }
                        !should_remove
                    });
                }
            }
        }
    }
}
