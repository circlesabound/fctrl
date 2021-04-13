use std::process::{ExitStatus, Stdio};

use log::{debug, error, info, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tokio::process::*;
use tokio::{io::AsyncBufReadExt, task::JoinHandle};

use crate::schema::ServerStartSaveFile;

pub mod builder {
    use std::{net::SocketAddr, path::Path};

    use tokio::process::Command;

    use crate::{factorio::Factorio, schema::ServerStartSaveFile, util};

    use super::*;

    pub struct ServerBuilder {
        cmd_builder: Command,
        savefile: Option<ServerStartSaveFile>,
    }

    impl ServerBuilder {
        pub fn using_installation(installation: &Factorio) -> ServerBuilder {
            let path_to_executable = installation
                .path
                .join("factorio")
                .join("bin")
                .join("x64")
                .join("factorio");
            ServerBuilder {
                cmd_builder: Command::new(path_to_executable),
                savefile: None,
            }
        }

        pub fn creating_savefile(self, new_savefile_name: &str) -> Self {
            self.with_cli_args(&[
                "--create",
                util::saves::get_savefile_path(new_savefile_name)
                    .to_str()
                    .unwrap(),
            ])
        }

        pub fn with_savefile(mut self, savefile: ServerStartSaveFile) -> Self {
            self.savefile.replace(savefile.clone());
            match savefile {
                ServerStartSaveFile::Latest => self.with_cli_args(&["--start-server-load-latest"]), // TODO this doesn't work with a custom save dir
                ServerStartSaveFile::Specific(savefile_name) => self.with_cli_args(&[
                    "--start-server",
                    util::saves::get_savefile_path(&savefile_name)
                        .to_str()
                        .unwrap(),
                ]),
            }
        }

        pub fn bind_on(self, server_bind_address: SocketAddr) -> Self {
            self.with_cli_args(&["--bind", &server_bind_address.to_string()])
        }

        pub fn with_rcon(self, rcon_settings: RconSettings) -> Self {
            self.with_cli_args(&[
                "--rcon-bind",
                &rcon_settings.bind.to_string(),
                "--rcon-password",
                &rcon_settings.password,
            ])
        }

        pub fn with_server_settings<P: AsRef<Path>>(self, server_settings_path: P) -> Self {
            self.with_cli_args(&[
                "--server-settings",
                server_settings_path.as_ref().to_str().unwrap(),
            ])
        }

        pub fn with_admin_list_file<P: AsRef<Path>>(self, admin_list_path: P) -> Self {
            self.with_cli_args(&[
                "--server-adminlist",
                admin_list_path.as_ref().to_str().unwrap(),
            ])
        }

        pub fn build(mut self) -> StartableInstance {
            // configure io to be piped
            self.cmd_builder
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // set this for a better night's sleep
            self.cmd_builder.kill_on_drop(true);

            StartableInstance {
                cmd: self.cmd_builder,
                savefile: self.savefile,
            }
        }

        pub fn with_cli_args<I, S>(mut self, args: I) -> Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<std::ffi::OsStr>,
        {
            self.cmd_builder.args(args);
            self
        }
    }

    pub struct RconSettings {
        bind: SocketAddr,
        password: String,
    }
}

pub mod proc {
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
}

pub struct StartableInstance {
    cmd: Command,
    savefile: Option<ServerStartSaveFile>,
}

impl StartableInstance {
    pub async fn start(mut self) -> crate::error::Result<RunningInstance> {
        let mut instance = self.cmd.spawn()?;
        info!(
            "Child process started with PID {}!",
            instance
                .id()
                .map_or("None".to_owned(), |pid| pid.to_string())
        );

        let out_stream = instance.stdout.take().unwrap();
        let err_stream = instance.stderr.take().unwrap();

        let handle_out = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(out_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("## Server stdout ## {}", line);
            }
            info!("## Server stdout end ##")
        });

        let handle_err = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(err_stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("## Server stderr ## {}", line);
            }
            info!("## Server stderr end ##")
        });

        Ok(RunningInstance {
            process: instance,
            savefile: self.savefile,
            handle_out,
            handle_err,
        })
    }
}

pub struct RunningInstance {
    process: Child,
    savefile: Option<ServerStartSaveFile>,
    handle_out: JoinHandle<()>,
    handle_err: JoinHandle<()>,
}

impl RunningInstance {
    /// Attempts to stop the instance by sending SIGTERM and waiting for the process to exit.
    ///
    /// # Errors
    ///
    /// This will only error in critical situations:
    /// - failed to find pid
    /// - sending SIGTERM failed
    /// - wait() on the process failed
    pub async fn stop(mut self) -> crate::error::Result<StoppedInstance> {
        if let Some(exit_status) = self.process.try_wait()? {
            // process already exited
            warn!(
                "Stop command received but child process already exited with status {}",
                exit_status
            );
            return Ok(StoppedInstance {
                exit_status,
                savefile: self.savefile,
            });
        }

        // Grab pid, this will fail in the unlikely case if process exits between the previous try_wait and now
        let pid = self
            .process
            .id()
            .ok_or(crate::error::Error::ProcessPidError)? as i32;

        // send SIGTERM to factorio child process
        // server will gracefully save and shut down
        if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
            error!(
                "Failed to send SIGTERM to child process with pid {}: {:?}",
                pid, e
            );
            return Err(crate::error::Error::ProcessSignalError(e));
        }

        self.wait().await
    }

    pub async fn wait(mut self) -> crate::error::Result<StoppedInstance> {
        let exit_status = self.process.wait().await?;
        info!("Child process exited with status {}", exit_status);

        // clean up piped output handlers
        self.handle_out.abort();
        self.handle_err.abort();

        Ok(StoppedInstance {
            exit_status,
            savefile: self.savefile,
        })
    }
}

pub struct StoppedInstance {
    pub exit_status: ExitStatus,
    pub savefile: Option<ServerStartSaveFile>,
}

/*

CLI configurability:

- savefile (portable location)
- mod folder (portable location)
- server settings file (portable location)
- server admin list file (portable location)
- server bind
- rcon bind
- rcon password

*/
