//! Runs the real `nama-server` binary.
#![cfg(unix)]

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Kills the server if the test fails before it exits, so no process is left behind.
struct KillOnDrop(Option<Child>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _: io::Result<()> = child.kill();
            let _: io::Result<ExitStatus> = child.wait();
        }
    }
}

/// Removes the temp dir however the test ends.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _: io::Result<()> = std::fs::remove_dir_all(&self.0);
    }
}

/// Returns the first log line containing `needle`, skipping earlier lines.
#[expect(
    clippy::panic,
    reason = "a missing log line fails the test that waits for it"
)]
fn wait_for_line(lines: &mpsc::Receiver<String>, needle: &str) -> String {
    loop {
        let line = lines
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|err| panic!("server never logged {needle:?}: {err}"));
        if line.contains(needle) {
            return line;
        }
    }
}

/// The address in a "listening" log line.
#[expect(
    clippy::panic,
    reason = "a line without an address fails the test that reads it"
)]
fn bound_addr(listening_line: &str) -> SocketAddr {
    listening_line
        .split_whitespace()
        .find_map(|word| word.strip_prefix("addr=")?.parse().ok())
        .unwrap_or_else(|| panic!("no address in {listening_line:?}"))
}

#[test]
fn server_logs_its_config_creates_its_database_enables_plugins_serves_the_api_and_exits_cleanly_on_ctrl_c()
 {
    let dir = std::env::temp_dir().join(format!("nama-server-binary-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = RemoveOnDrop(dir);
    let config_file = dir.0.join("server.toml");
    let data_dir = dir.0.join("data");
    std::fs::write(
        &config_file,
        format!(
            "name = \"From File\"\ndata_dir = \"{}\"\n",
            data_dir.display()
        ),
    )
    .unwrap();

    let mut server = KillOnDrop(Some(
        Command::new(env!("CARGO_BIN_EXE_nama-server"))
            .arg("--config")
            .arg(&config_file)
            .env("NAMA_LISTEN", "127.0.0.1:0")
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let child = server.0.as_mut().unwrap();

    // Read log lines on a thread so a hung server fails the test instead of blocking it.
    let (lines_tx, lines) = mpsc::channel();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    thread::spawn(move || {
        for line in stdout.lines().map_while(Result::ok) {
            let _: Result<(), mpsc::SendError<String>> = lines_tx.send(line);
        }
    });

    let config_line = wait_for_line(&lines, "loaded config");
    assert!(config_line.contains("127.0.0.1:0"), "{config_line}");
    assert!(
        config_line.contains(&data_dir.display().to_string()),
        "{config_line}"
    );

    // A fresh data directory gets a new database with every migration applied.
    let database_line = wait_for_line(&lines, "opened database");
    assert!(
        database_line.contains("migrations_applied=[1]"),
        "{database_line}"
    );
    assert!(data_dir.join("nama.db").is_file());

    // Port 0 asks the OS for a free port; the Server logs the one it got.
    let addr = bound_addr(&wait_for_line(&lines, "listening"));

    // The log-events Built-in Plugin is enabled by default and observes the start.
    let observed_line = wait_for_line(&lines, "observed server started");
    assert!(observed_line.contains("From File"), "{observed_line}");

    // HTTP/1.0 so the response isn't chunked and ends when the connection closes.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        stream,
        "POST /nama.v1.ServerService/GetServerInfo HTTP/1.0\r\n\
         Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains(" 200 "), "{response}");
    assert!(response.contains(r#""name":"From File""#), "{response}");

    let kill = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(kill.success());

    let (exited_tx, exited) = mpsc::channel();
    let exiting = server.0.take().unwrap();
    thread::spawn(move || exited_tx.send(exiting.wait_with_output().unwrap()));
    let output = exited
        .recv_timeout(TIMEOUT)
        .expect("server did not exit after Ctrl-C");
    assert!(
        output.status.success(),
        "server exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
