use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    error::{Error, Result},
    server::{
        builder::{StartableInstanceBuilder, StartableShortLivedInstanceBuilder},
        *,
    },
};

pub struct ProcessManager {
    running_instance: Arc<Mutex<Option<StartedInstance>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        ProcessManager {
            running_instance: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn status(&self) -> ProcessStatus {
        self.instance_is_running_or_cleanup().await;
        self.internal_status().await
    }

    pub async fn start_instance<B: StartableInstanceBuilder>(&self, builder: B) -> Result<()> {
        let mut mg = self.running_instance.lock().await;

        if mg.is_some() {
            return Err(Error::ProcessAlreadyRunning);
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

    pub async fn start_and_wait_for_shortlived_instance<B: StartableShortLivedInstanceBuilder>(
        &self,
        builder: B,
    ) -> Result<StoppedShortLivedInstance> {
        // hold mutex to prevent anything else from running
        let mg = self.running_instance.lock().await;

        if mg.is_some() {
            return Err(Error::ProcessAlreadyRunning);
        }

        let startable = builder.build();
        let stopped = startable.start_and_wait().await?;

        Ok(stopped)
    }

    async fn internal_status(&self) -> ProcessStatus {
        let mg = self.running_instance.lock().await;
        if let Some(started) = mg.as_ref() {
            ProcessStatus::Running(started.get_internal_server_state().await)
        } else {
            ProcessStatus::NotRunning
        }
    }

    async fn instance_is_running_or_cleanup(&self) -> bool {
        let mut mg = self.running_instance.lock().await;
        if let Some(running) = mg.as_mut() {
            match running.poll_process_exited().await {
                Err(e) => {
                    // log and ignore for now, use in-process status
                    error!("Error polling process status: {:?}", e);
                    return true;
                }
                Ok(false) => {
                    // process still running
                    return true;
                }
                Ok(true) => {
                    // polled result shows process exited, update our status
                    // code path continues after `else`
                }
            }
        } else {
            // not running to begin with
            return false;
        }

        // Continuation from mismatched polled result
        // Manually wait (should be no-op), and drop StoppedInstance
        warn!("Detected premature process exited");
        let _ = mg.take().unwrap().wait().await;
        false
    }
}

pub enum ProcessStatus {
    NotRunning,
    Running(InternalServerState),
}
