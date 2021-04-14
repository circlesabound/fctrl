use std::process::ExitStatus;

use log::{error, info, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tokio::process::*;
use tokio::{io::AsyncBufReadExt, task::JoinHandle};

use crate::schema::ServerStartSaveFile;

use settings::LaunchSettings;

pub mod builder;
pub mod proc;
pub mod settings;

pub struct StartableInstance {
    cmd: Command,
    launch_settings: Option<LaunchSettings>,
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
            launch_settings: self.launch_settings,
            savefile: self.savefile,
            handle_out,
            handle_err,
        })
    }
}

pub struct RunningInstance {
    process: Child,
    launch_settings: Option<LaunchSettings>,
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
                launch_settings: self.launch_settings,
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
            launch_settings: self.launch_settings,
            savefile: self.savefile,
        })
    }
}

pub struct StoppedInstance {
    pub exit_status: ExitStatus,
    pub launch_settings: Option<LaunchSettings>,
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
