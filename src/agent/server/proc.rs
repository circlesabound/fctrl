use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt},
    sync::{Mutex, RwLock},
};

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

        // TODO extract the auto-pause setting here and store, then disable RCON when players == 0

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

    pub async fn send_rcon_command_to_instance(&self, cmd: &str) -> Result<String> {
        let mg = self.running_instance.lock().await;
        if let Some(instance) = mg.as_ref() {
            if let Some(rcon) = instance.get_rcon().await.as_ref() {
                Ok(rcon.send(cmd).await?)
            } else {
                Err(Error::RconNotConnected)
            }
        } else {
            Err(Error::ProcessNotRunning)
        }
    }

    async fn internal_status(&self) -> ProcessStatus {
        let mg = self.running_instance.lock().await;
        if let Some(started) = mg.as_ref() {
            ProcessStatus::Running {
                player_count: started.get_player_count(),
                server_state: started.get_internal_server_state().await,
            }
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
                    true
                }
                Ok(false) => {
                    // process still running
                    true
                }
                Ok(true) => {
                    // polled result shows process exited, update our status
                    // Manually wait (should be no-op), and drop StoppedInstance
                    warn!("Detected premature process exited");
                    let _ = mg.take().unwrap().wait().await; // safe since we hold the mutex guard
                    false
                }
            }
        } else {
            // not running to begin with
            false
        }
    }
}

pub enum ProcessStatus {
    NotRunning,
    Running {
        player_count: u32,
        server_state: InternalServerState,
    },
}

pub async fn parse_process_stdout(
    lines_reader: impl AsyncBufRead + Unpin,
    stdout_handler: Box<dyn HandlerFn>,
    rcon: Arc<RwLock<Option<Rcon>>>,
    rcon_password: String,
    rcon_bind: SocketAddr,
    internal_server_state: Arc<RwLock<InternalServerState>>,
    player_count: Arc<AtomicU32>,
) {
    let mut rcon_initialised = false;
    let mut lines = lines_reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        // Parse for internal server state (whether the game is running, stopped, etc)
        lazy_static! {
            static ref STATE_CHANGE_RE: Regex =
                Regex::new(r"changing state from\(([a-zA-Z]+)\) to\(([a-zA-Z]+)\)").unwrap();
        }
        if let Some(captures) = STATE_CHANGE_RE.captures(&line) {
            if let Ok(from) = InternalServerState::from_str(captures.get(1).unwrap().as_str()) {
                if let Ok(to) = InternalServerState::from_str(captures.get(2).unwrap().as_str()) {
                    info!(
                        "Server switching internal state from {:?} to {:?}",
                        from, to
                    );
                    *internal_server_state.write().await = to;
                } else {
                    warn!(
                        "Server internal state change but could not parse 'to' state from log: {}",
                        line
                    );
                }
            } else {
                warn!(
                    "Server internal state change but could not parse 'from' state from log: {}",
                    line
                );
            }
        }

        // Parse for player join / leave, update counter
        lazy_static! {
            static ref JOIN_RE: Regex = Regex::new(
                r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+) joined the game$"
            )
            .unwrap();
            static ref LEAVE_RE: Regex = Regex::new(
                r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$"
            )
            .unwrap();
        }
        if JOIN_RE.is_match(&line) {
            player_count.fetch_add(1, Ordering::Relaxed);
        } else if LEAVE_RE.is_match(&line) {
            player_count.fetch_sub(1, Ordering::Relaxed);
        }

        // If not already open, parse for "RCON ready message", then attempt to connect
        lazy_static! {
            static ref RCON_READY_RE: Regex =
                Regex::new(r"Starting RCON interface at IP ADDR:\(\{\d+\.\d+\.\d+\.\d+:(\d+)\}\)")
                    .unwrap();
        }
        if !rcon_initialised {
            if let Some(captures) = RCON_READY_RE.captures(&line) {
                match u16::from_str(captures.get(1).unwrap().as_str()) {
                    Ok(port) => {
                        if port != rcon_bind.port() {
                            warn!("RCON bound port was configured to be {}, but Factorio is using port {} instead!", rcon_bind.port(), port);
                        }
                        match Rcon::connect(
                            SocketAddrV4::new(Ipv4Addr::LOCALHOST, port),
                            &rcon_password,
                        )
                        .await
                        {
                            Ok(rcon_conn) => {
                                *rcon.write().await = Some(rcon_conn);
                                rcon_initialised = true;
                            }
                            Err(e) => {
                                error!("Error connecting to RCON: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error parsing RCON ready stdout message: {:?}", e);
                    }
                }
            }
        }

        // Pass off to stdout handler
        (stdout_handler)(line);
    }
}
