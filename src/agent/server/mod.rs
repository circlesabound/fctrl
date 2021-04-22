use std::str::FromStr;
use std::sync::Arc;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    process::ExitStatus,
};

use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use regex::Regex;
use strum_macros::EnumString;
use tokio::io::AsyncBufReadExt;
use tokio::process::*;
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use fctrl::schema::ServerStartSaveFile;

use settings::*;

use self::rcon::Rcon;

pub mod builder;
pub mod mods;
pub mod proc;
pub mod rcon;
pub mod settings;

pub trait HandlerFn = Fn(String) + Send + Sync + 'static;

pub struct StartableInstance {
    cmd: Command,
    stdout_handler: Box<dyn HandlerFn>,
    admin_list: AdminList,
    launch_settings: LaunchSettings,
    savefile: ServerStartSaveFile,
    server_settings: ServerSettings,
    _optional_args: Vec<String>,
}

impl StartableInstance {
    pub async fn start(mut self) -> Result<StartedInstance> {
        let mut instance = self.cmd.spawn()?;
        info!(
            "Child process started with PID {}!",
            instance
                .id()
                .map_or("None".to_owned(), |pid| pid.to_string())
        );

        let rcon = Arc::new(RwLock::new(None));
        let rcon_arc = Arc::clone(&rcon);
        let configured_rcon_bind = self.launch_settings.rcon_bind.clone();
        let rcon_password_clone = self.launch_settings.rcon_password.clone();

        let out_stream = instance.stdout.take().unwrap();
        let err_stream = instance.stderr.take().unwrap();

        let internal_server_state = Arc::new(RwLock::new(InternalServerState::Ready));
        let internal_server_state_arc = Arc::clone(&internal_server_state);
        let internal_stdout_handler = self.stdout_handler;
        tokio::spawn(async move {
            let mut rcon_initialised = false;

            let mut lines = tokio::io::BufReader::new(out_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Parse for internal server state
                lazy_static! {
                    static ref STATE_CHANGE_RE: Regex =
                        Regex::new(r"changing state from\(([a-zA-Z]+)\) to\(([a-zA-Z]+)\)")
                            .unwrap();
                }
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
                            *internal_server_state_arc.write().await = to;
                        } else {
                            warn!("Server internal state change but could not parse 'to' state from log: {}", line);
                        }
                    } else {
                        warn!("Server internal state change but could not parse 'from' state from log: {}", line);
                    }
                }

                // If not already open, parse for "RCON ready message"
                lazy_static! {
                    static ref RCON_READY_RE: Regex = Regex::new(
                        r"Starting RCON interface at IP ADDR:\(\{\d+\.\d+\.\d+\.\d+:(\d+)\}\)"
                    )
                    .unwrap();
                }
                if !rcon_initialised {
                    if let Some(captures) = RCON_READY_RE.captures(&line) {
                        match u16::from_str(captures.get(1).unwrap().as_str()) {
                            Ok(port) => {
                                if port != configured_rcon_bind.port() {
                                    warn!("RCON bound port was configured to be {}, but Factorio is using port {} instead!", configured_rcon_bind.port(), port);
                                }
                                match Rcon::connect(
                                    SocketAddrV4::new(Ipv4Addr::LOCALHOST, port),
                                    &rcon_password_clone,
                                )
                                .await
                                {
                                    Ok(rcon) => {
                                        *rcon_arc.write().await = Some(rcon);
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
                (internal_stdout_handler)(line);
            }
        });

        tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(err_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Not sure if Factorio executable logs anything to stderr
                error!("## Server stderr ## {}", line);
            }
        });

        Ok(StartedInstance {
            process: instance,
            rcon,
            internal_server_state,
            admin_list: self.admin_list,
            launch_settings: self.launch_settings,
            savefile: self.savefile,
            server_settings: self.server_settings,
            _optional_args: self._optional_args,
        })
    }
}

pub struct StartedInstance {
    process: Child,
    rcon: Arc<RwLock<Option<Rcon>>>,
    internal_server_state: Arc<RwLock<InternalServerState>>,
    admin_list: AdminList,
    launch_settings: LaunchSettings,
    savefile: ServerStartSaveFile,
    server_settings: ServerSettings,
    _optional_args: Vec<String>,
}

impl StartedInstance {
    /// Attempts to stop the instance by sending SIGTERM and waiting for the process to exit.
    ///
    /// # Errors
    ///
    /// This will only error in critical situations:
    /// - failed to find pid
    /// - sending SIGTERM failed
    /// - wait() on the process failed
    pub async fn stop(mut self) -> Result<StoppedInstance> {
        if let Some(exit_status) = self.process.try_wait()? {
            // process already exited
            warn!(
                "Stop command received but child process already exited with status {}",
                exit_status
            );
            return Ok(StoppedInstance {
                exit_status,
                admin_list: self.admin_list,
                launch_settings: self.launch_settings,
                savefile: self.savefile,
                server_settings: self.server_settings,
                _optional_args: self._optional_args,
            });
        }

        // Grab pid, this will fail in the unlikely case if process exits between the previous try_wait and now
        let pid = self.process.id().ok_or(Error::ProcessPidError)? as i32;

        // send SIGTERM to factorio child process
        // server will gracefully save and shut down
        if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
            error!(
                "Failed to send SIGTERM to child process with pid {}: {:?}",
                pid, e
            );
            return Err(Error::ProcessSignalError(e));
        }

        self.wait().await
    }

    pub async fn wait(mut self) -> Result<StoppedInstance> {
        let exit_status = self.process.wait().await?;
        info!("Child process exited with status {}", exit_status);

        Ok(StoppedInstance {
            exit_status,
            admin_list: self.admin_list,
            launch_settings: self.launch_settings,
            savefile: self.savefile,
            server_settings: self.server_settings,
            _optional_args: self._optional_args,
        })
    }

    /// Manually poll whether the child process has exited
    pub async fn poll_process_exited(&mut self) -> Result<bool> {
        Ok(self.process.try_wait()?.is_some())
    }

    pub async fn get_internal_server_state(&self) -> InternalServerState {
        self.internal_server_state.read().await.clone()
    }

    pub async fn get_rcon(&self) -> tokio::sync::RwLockReadGuard<'_, Option<Rcon>> {
        self.rcon.read().await
    }
}

pub struct StoppedInstance {
    pub exit_status: ExitStatus,
    pub admin_list: AdminList,
    pub launch_settings: LaunchSettings,
    pub savefile: ServerStartSaveFile,
    pub server_settings: ServerSettings,
    pub _optional_args: Vec<String>,
}

pub struct StartableShortLivedInstance {
    cmd: Command,
    stdout_handler: Box<dyn HandlerFn>,
}

impl StartableShortLivedInstance {
    pub async fn start_and_wait(mut self) -> Result<StoppedShortLivedInstance> {
        let mut instance = self.cmd.spawn()?;
        info!(
            "Child process started with PID {}!",
            instance
                .id()
                .map_or("None".to_owned(), |pid| pid.to_string())
        );

        let out_stream = instance.stdout.take().unwrap();
        let err_stream = instance.stderr.take().unwrap();

        let internal_stdout_handler = self.stdout_handler;
        let handle_out = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(out_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Pass off to stdout handler
                (internal_stdout_handler)(line);
            }
        });

        let handle_err = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(err_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Not sure if Factorio executable logs anything to stderr
                error!("## Short-lived instance stderr ## {}", line);
            }
        });

        let exit_status = instance.wait().await?;
        info!("Child process exited with status {}", exit_status);

        // clean up piped output handlers
        handle_out.abort();
        handle_err.abort();

        Ok(StoppedShortLivedInstance { exit_status })
    }
}

pub struct StoppedShortLivedInstance {
    pub exit_status: ExitStatus,
}

/// Internal state of the Factorio multiplayer server as tracked by output logs
#[derive(Clone, Debug, EnumString)]
pub enum InternalServerState {
    Ready,
    PreparedToHostGame,
    CreatingGame,
    InGame,
    InGameSavingMap,
    DisconnectingScheduled,
    Disconnecting,
    Disconnected,
    Closed,
}
