use std::process::Stdio;

use tokio::process::*;

use crate::factorio::Factorio;

struct Server {
    command: String,
    process: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
}

impl Server {
    fn start(installation: &Factorio) -> crate::error::Result<Self> {
        let mut cmd_builder = Command::new(&installation.path);

        let args = vec!["test".to_owned()];

        cmd_builder
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut process = cmd_builder.spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();
        let stderr = process.stderr.take().unwrap();

        Ok(Server {
            command: "test".to_owned(),
            process,
            stdin,
            stdout,
            stderr,
        })
    }
}

enum SaveFile {
    Latest,
    Specific(String),
}

/*

CLI configurability:

- savefile (portable location)
- mod folder (portable location)
- server settings file (portable location)
- server admin list file (portable location)
- server port
- rcon port
- rcon password

*/
