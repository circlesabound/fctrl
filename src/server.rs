use std::process::Stdio;

use log::{debug, error, info, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tokio::process::*;
use tokio::{io::AsyncBufReadExt, task::JoinHandle};

pub mod builder {
    use std::{net::SocketAddr, path::Path};

    use tokio::process::Command;

    use crate::{factorio::Factorio, schema::ServerStartSaveFile, util};

    use super::*;

    pub struct ServerBuilder {
        cmd_builder: Command,
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
            }
        }

        pub fn creating_savefile<P: AsRef<Path>>(self, new_savefile_path: P) -> Self {
            self.with_cli_args(&["--create", new_savefile_path.as_ref().to_str().unwrap()])
        }

        pub fn with_savefile(self, savefile: ServerStartSaveFile) -> Self {
            match savefile {
                ServerStartSaveFile::Latest => self.with_cli_args(&["--start-server-load-latest"]),
                ServerStartSaveFile::Specific(savefile_name) => self.with_cli_args(&[
                    "--start-server",
                    util::saves::get_savefile_path(savefile_name)
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

            StartableInstance {
                cmd: self.cmd_builder,
            }
        }

        fn with_cli_args<I, S>(mut self, args: I) -> Self
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

pub struct StartableInstance {
    cmd: Command,
}

impl StartableInstance {
    pub async fn start(mut self) -> crate::error::Result<RunningInstance> {
        let mut instance = self.cmd.spawn()?;

        // TODO connect ostream and errstream
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
            handle_out,
            handle_err,
        })
    }
}

pub struct RunningInstance {
    process: Child,
    handle_out: JoinHandle<()>,
    handle_err: JoinHandle<()>,
}

impl RunningInstance {
    pub async fn stop(mut self) -> crate::error::Result<StoppedInstance> {
        if let Some(exit_status) = self.process.try_wait()? {
            // process already exited
            warn!(
                "Stop command received but child process already exited with status {}",
                exit_status
            );
            return Err(crate::error::Error::ProcessAlreadyExited);
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

        let exit_status = self.process.wait().await?;
        info!("Child process exited with status {}", exit_status);

        // clean up piped output handlers
        self.handle_out.abort();
        self.handle_err.abort();

        Ok(StoppedInstance {})
    }

    /// Use for short-running instances e.g. create savefile
    pub async fn wait(mut self) -> crate::error::Result<StoppedInstance> {
        let exit_status = self.process.wait().await?;
        info!("Child process exited with status {}", exit_status);

        // clean up piped output handlers
        self.handle_out.abort();
        self.handle_err.abort();

        Ok(StoppedInstance {})
    }
}

pub struct StoppedInstance {
    //
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
