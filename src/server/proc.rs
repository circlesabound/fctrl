use std::sync::Arc;

use tokio::sync::Mutex;

use super::{builder::ServerBuilder, *};

pub struct ProcessManager {
    running_instance: Arc<Mutex<Option<RunningInstance>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        ProcessManager {
            running_instance: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn run_instance(&self, builder: ServerBuilder) -> crate::error::Result<()> {
        let mut mg = self.running_instance.lock().await;

        if mg.is_some() {
            return Err(crate::error::Error::ProcessAlreadyRunning);
        }

        let startable = builder.build();
        let running = startable.start().await?;
        mg.replace(running);

        Ok(())
    }

    pub async fn stop_instance(&self) -> Option<StoppedInstance> {
        let mut mg = self.running_instance.lock().await;

        match mg.take() {
            None => None,
            Some(running) => {
                match running.stop().await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        // Could not stop the instance for whatever reason (should never happen).
                        // Tricky to deal with. For now we just drop the instance and hope the
                        // underlying process exits and cleans up eventually
                        error!("Failed to stop instance, ignoring failure and dropping process handles. Error: {:?}", e);
                        None
                    }
                }
            }
        }
    }

    pub async fn wait_for_instance(&self) -> Option<StoppedInstance> {
        let mut mg = self.running_instance.lock().await;

        match mg.take() {
            None => None,
            Some(running) => {
                match running.wait().await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        // Could not wait for whatever reason (should never happen).
                        // Tricky to deal with. For now we just drop the instance and hope the
                        // underlying process exits and cleans up eventually
                        error!("Failed to wait for instance, ignoring failure and dropping process handles. Error: {:?}", e);
                        None
                    }
                }
            }
        }
    }

    pub async fn instance_is_running(&self) -> bool {
        let mg = self.running_instance.lock().await;
        mg.is_some()
    }
}
