use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use log::debug;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
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
use fctrl::schema::regex::*;

pub struct ProcessManager {
    sysinfo: Arc<RwLock<System>>,
    running_instance: Arc<Mutex<Option<StartedInstance>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        let sysinfo_refresh_specifics = RefreshKind::new()
            .with_cpu(CpuRefreshKind::new().with_cpu_usage())
            .with_memory(MemoryRefreshKind::new().with_ram());
        let sysinfo = Arc::new(RwLock::new(System::new_with_specifics(sysinfo_refresh_specifics)));
        let sysinfo_arc = Arc::clone(&sysinfo);
        tokio::spawn(async move {
            loop {
                // refresh system stats every 10 seconds
                tokio::time::sleep(Duration::from_secs(10)).await;
                if let Ok(mut sysinfo) = tokio::time::timeout(Duration::from_millis(250), sysinfo_arc.write()).await {
                    sysinfo.refresh_specifics(sysinfo_refresh_specifics);
                } else {
                    warn!("Unable to acquire write lock for sysinfo, skipping this cycle");
                }
            }
        });
        ProcessManager {
            sysinfo,
            running_instance: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn system_resources(&self) -> Result<SystemResources> {
        if let Ok(sysinfo) = tokio::time::timeout(Duration::from_millis(250), self.sysinfo.read()).await {
            Ok(SystemResources {
                cpu_total: sysinfo.global_cpu_usage(),
                cpus: sysinfo.cpus().into_iter().map(|cpu| cpu.cpu_usage()).collect(),
                mem_total_bytes: sysinfo.total_memory(),
                mem_used_bytes: sysinfo.used_memory(),
            })
        } else {
            Err(Error::Timeout)
        }
    }

    pub async fn status(&self) -> ProcessStatus {
        if !self.instance_is_running_or_cleanup().await {
            ProcessStatus::NotRunning
        } else {
            self.internal_status().await
        }
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

    pub async fn _wait_for_instance(&self) -> Option<StoppedInstance> {
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
    loop {
        match lines.next_line().await {
            Ok(line_opt) => {
                if let Some(line) = line_opt {
                    // Parse for internal server state (whether the game is running, stopped, etc)
                    if let Some(captures) = STATE_CHANGE_RE.captures(&line) {
                        if let Ok(from) =
                            InternalServerState::from_str(captures.get(1).unwrap().as_str())
                        {
                            if let Ok(to) =
                                InternalServerState::from_str(captures.get(2).unwrap().as_str())
                            {
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
                    if JOIN_RE.is_match(&line) {
                        player_count.fetch_add(1, Ordering::Relaxed);
                    } else if LEAVE_RE.is_match(&line) {
                        player_count.fetch_sub(1, Ordering::Relaxed);
                    }

                    // If not already open, parse for "RCON ready message", then attempt to connect
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
                } else {
                    // None means end of stream
                    break;
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    // This happens when you try to use windows emoji keyboard in the in-game chat
                    debug!("Invalid UTF-8 encountered while reading line in parse_process_stdout, skipping. Error: {:?}", e);
                } else {
                    warn!(
                        "parse_process_stdout got unexpected error: {:?}. Breaking out of loop",
                        e
                    );
                    break;
                }
            }
        }
    }
}
