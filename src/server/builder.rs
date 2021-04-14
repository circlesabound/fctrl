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
