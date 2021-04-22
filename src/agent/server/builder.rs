use std::process::Stdio;

use tokio::process::Command;

use crate::{factorio::Factorio, util};
use fctrl::schema::ServerStartSaveFile;

use super::{
    mods::ModManager,
    settings::{AdminList, LaunchSettings, ServerSettings},
    HandlerFn, StartableInstance, StartableShortLivedInstance, StoppedInstance,
};

pub trait StartableInstanceBuilder {
    fn replay_optional_args(&mut self, previous_instance: StoppedInstance) -> &Self;
    fn build(self) -> StartableInstance;
}

pub trait StartableShortLivedInstanceBuilder {
    fn build(self) -> StartableShortLivedInstance;
}

pub struct ServerBuilder {
    cmd_builder: Command,
    stdout_handler: Box<dyn HandlerFn>,
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
            stdout_handler: Box::new(ServerBuilder::noop_stdout_handler),
        }
    }

    pub fn with_stdout_handler<H: HandlerFn>(mut self, stdout_handler: H) -> ServerBuilder {
        self.stdout_handler = Box::new(stdout_handler);
        self
    }

    pub fn creating_savefile(mut self, new_savefile_name: String) -> SaveCreatorBuilder {
        self.with_cli_args(&[
            "--create",
            util::saves::get_savefile_path(&new_savefile_name)
                .to_str()
                .unwrap(),
        ]);
        SaveCreatorBuilder {
            cmd_builder: self.cmd_builder,
            stdout_handler: self.stdout_handler,
            _new_savefile_name: new_savefile_name,
        }
    }

    pub fn hosting_savefile(
        mut self,
        savefile: ServerStartSaveFile,
        mods: ModManager,
        admin_list: AdminList,
        launch_settings: LaunchSettings,
        server_settings: ServerSettings,
    ) -> ServerHostBuilder {
        match &savefile {
            ServerStartSaveFile::Latest => self.with_cli_args(&["--start-server-load-latest"]), // TODO this doesn't work with a custom save dir
            ServerStartSaveFile::Specific(savefile_name) => self.with_cli_args(&[
                "--start-server",
                util::saves::get_savefile_path(&savefile_name)
                    .to_str()
                    .unwrap(),
            ]),
        };

        self.with_cli_args(&[
            "--bind",
            &launch_settings.server_bind.to_string(),
            "--rcon-bind",
            &launch_settings.rcon_bind.to_string(),
            "--rcon-password",
            &launch_settings.rcon_password,
        ]);

        self.with_cli_args(&["--server-settings", server_settings.path.to_str().unwrap()]);

        self.with_cli_args(&["--server-adminlist", admin_list.path.to_str().unwrap()]);

        self.with_cli_args(&["--mod-directory", mods.path.to_str().unwrap()]);

        ServerHostBuilder {
            cmd_builder: self.cmd_builder,
            stdout_handler: self.stdout_handler,
            admin_list,
            launch_settings,
            savefile,
            server_settings,
            _optional_args: vec![],
        }
    }

    fn with_cli_args<I, S>(&mut self, args: I) -> &Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.cmd_builder.args(args);
        self
    }

    fn noop_stdout_handler(s: String) {
        // do nothing
    }
}

pub struct ServerHostBuilder {
    cmd_builder: Command,
    stdout_handler: Box<dyn HandlerFn>,
    admin_list: AdminList,
    launch_settings: LaunchSettings,
    savefile: ServerStartSaveFile,
    server_settings: ServerSettings,
    _optional_args: Vec<String>,
}

impl StartableInstanceBuilder for ServerHostBuilder {
    fn replay_optional_args(&mut self, previous_instance: StoppedInstance) -> &Self {
        self._optional_args.extend(previous_instance._optional_args);
        self
    }

    fn build(mut self) -> StartableInstance {
        // configure io to be piped
        self.cmd_builder
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // set this for a better night's sleep
        self.cmd_builder.kill_on_drop(true);

        StartableInstance {
            cmd: self.cmd_builder,
            stdout_handler: self.stdout_handler,
            admin_list: self.admin_list,
            launch_settings: self.launch_settings,
            savefile: self.savefile,
            server_settings: self.server_settings,
            _optional_args: self._optional_args,
        }
    }
}

pub struct SaveCreatorBuilder {
    cmd_builder: Command,
    stdout_handler: Box<dyn HandlerFn>,
    _new_savefile_name: String,
}

impl StartableShortLivedInstanceBuilder for SaveCreatorBuilder {
    fn build(mut self) -> StartableShortLivedInstance {
        // configure io to be piped
        self.cmd_builder
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // set this for a better night's sleep
        self.cmd_builder.kill_on_drop(true);

        StartableShortLivedInstance {
            cmd: self.cmd_builder,
            stdout_handler: self.stdout_handler,
        }
    }
}
