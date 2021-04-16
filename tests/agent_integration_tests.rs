use std::time::Duration;
use std::{path::PathBuf, process::Stdio};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

use fctrl::{schema::*, util};

struct AgentTestFixture {
    agent: Child,
    client: Child,
    client_stdin: ChildStdin,
    client_stdout_lines: Lines<BufReader<ChildStdout>>,
}

impl AgentTestFixture {
    pub async fn new() -> Self {
        let executables_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("target")
            .join("debug");
        let agent = Command::new(executables_dir.join("agent"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        let mut client = Command::new(executables_dir.join("ws-client"))
            .arg("ws://localhost:5463")
            .arg("--pipe-mode")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let client_stdin = client.stdin.take().unwrap();
        let client_stdout_lines = BufReader::new(client.stdout.take().unwrap()).lines();

        AgentTestFixture {
            agent,
            client,
            client_stdin,
            client_stdout_lines,
        }
    }

    pub async fn client_writeln(&mut self, m: String) {
        let line = m + "\n";
        self.client_stdin.write_all(line.as_bytes()).await.unwrap();
    }

    pub async fn client_wait_for_final_reply(&mut self) -> std::io::Result<AgentResponseWithId> {
        loop {
            match self.client_stdout_lines.next_line().await {
                Ok(Some(line)) => {
                    let json = line.trim();
                    if let Ok(reply) = serde_json::from_str::<AgentResponseWithId>(json) {
                        match &reply.status {
                            OperationStatus::Ongoing => continue,
                            OperationStatus::Completed | OperationStatus::Failed => {
                                return Ok(reply)
                            }
                        }
                    } else {
                        // ignore?
                    }
                }
                Ok(None) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "got none value",
                    ));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for AgentTestFixture {
    fn drop(&mut self) {
        // send SIGTERM to agent
        signal::kill(
            Pid::from_raw(self.agent.id().unwrap() as i32),
            Signal::SIGTERM,
        )
        .unwrap();
    }
}

#[tokio::test]
async fn test_request_server_status() {
    util::testing::logger_init();

    let mut f = AgentTestFixture::new().await;

    f.client_writeln("ServerStatus".to_owned()).await;
    let response = timeout(
        Duration::from_millis(500),
        f.client_wait_for_final_reply()
    ).await.unwrap().unwrap();
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(response.content, AgentResponse::ServerStatus(ServerStatus { running: false })));
}
