use std::time::Duration;
use std::{path::PathBuf, process::Stdio};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use serial_test::serial;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};

use fctrl::{schema::*, util};

const VERSION_TO_INSTALL: &'static str = "1.1.32";

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
            .env("AGENT_WS_PORT", "5463")
            .env("FACTORIO_PORT", "34197")
            .env("FACTORIO_RCON_PORT", "27015")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()) // Comment out this line to show agent logs for debugging
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

    pub async fn client_wait_for_final_reply(&mut self, timeout: Duration) -> AgentResponseWithId {
        tokio::time::timeout(timeout, self.internal_client_wait_for_final_reply())
            .await
            .unwrap()
            .unwrap()
    }

    async fn internal_client_wait_for_final_reply(
        &mut self,
    ) -> std::io::Result<AgentResponseWithId> {
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
        // send SIGINT to agent
        signal::kill(
            Pid::from_raw(self.agent.id().unwrap() as i32),
            Signal::SIGINT,
        )
        .unwrap();

        // send SIGKILL to ws-client
        self.client.start_kill().unwrap();
    }
}

#[tokio::test]
#[serial]
async fn can_request_server_status() {
    util::testing::logger_init();

    let mut f = AgentTestFixture::new().await;

    f.client_writeln("ServerStatus".to_owned()).await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(
        response.content,
        AgentResponse::ServerStatus(ServerStatus::NotRunning)
    ));

    drop(f);
}

#[tokio::test]
#[serial]
async fn can_set_then_get_admin_list() {
    util::testing::logger_init();

    let mut f = AgentTestFixture::new().await;

    f.client_writeln(format!("VersionInstall {}", VERSION_TO_INSTALL))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_secs(120))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);

    let new_list = "admin1 admin2".to_owned();
    f.client_writeln(format!("ConfigAdminListSet {}", new_list))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(response.content, AgentResponse::Ok));

    f.client_writeln("ConfigAdminListGet".to_owned()).await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(
        response.content,
        AgentResponse::ConfigAdminList(list) if list.len() == 2 && list.contains(&"admin1".to_owned()) && list.contains(&"admin2".to_owned())
    ));

    drop(f);
}

#[tokio::test]
#[serial]
async fn can_set_then_get_rcon_config() {
    util::testing::logger_init();

    let mut f = AgentTestFixture::new().await;

    f.client_writeln(format!("VersionInstall {}", VERSION_TO_INSTALL))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_secs(120))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);

    let new_password = "newpassword".to_owned();
    f.client_writeln(format!("ConfigRconSet {}", new_password))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(response.content, AgentResponse::Ok));

    f.client_writeln("ConfigRconGet".to_owned()).await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(
        response.content,
        AgentResponse::ConfigRcon {
            port: 27015, password } if password == new_password
    ));

    drop(f);
}

#[tokio::test]
#[serial]
async fn can_set_then_get_server_settings() {
    util::testing::logger_init();

    let mut f = AgentTestFixture::new().await;

    f.client_writeln(format!("VersionInstall {}", VERSION_TO_INSTALL))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_secs(120))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);

    let new_server_settings = String::from(
        r#"{"name":"test123","description":"test123","visibility":{"public":false,"lan":false}}"#,
    );
    f.client_writeln(format!("ConfigServerSettingsSet {}", new_server_settings))
        .await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(response.content, AgentResponse::Ok));

    f.client_writeln("ConfigServerSettingsGet".to_owned()).await;
    let response = f
        .client_wait_for_final_reply(Duration::from_millis(500))
        .await;
    assert_eq!(response.status, OperationStatus::Completed);
    assert!(matches!(
        response.content,
        AgentResponse::ConfigServerSettings(json) if json == new_server_settings
    ));

    drop(f);
}
