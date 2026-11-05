//! Runs the real `nama-server` binary.
#![cfg(unix)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
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
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Removes the temp dir however the test ends.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn server_logs_its_config_serves_the_api_and_exits_cleanly_on_ctrl_c() {
    let dir = std::env::temp_dir().join(format!("nama-server-binary-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = RemoveOnDrop(dir);
    let config_file = dir.0.join("server.toml");
    std::fs::write(
        &config_file,
        "name = \"From File\"\ndata_dir = \"/srv/from-file\"\n",
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
            let _ = lines_tx.send(line);
        }
    });

    let config_line = loop {
        let line = lines
            .recv_timeout(TIMEOUT)
            .expect("server never logged its config");
        if line.contains("loaded config") {
            break line;
        }
    };
    assert!(config_line.contains("127.0.0.1:0"), "{config_line}");
    assert!(config_line.contains("/srv/from-file"), "{config_line}");

    // Port 0 asks the OS for a free port; the Server logs the one it got.
    let listening_line = loop {
        let line = lines
            .recv_timeout(TIMEOUT)
            .expect("server never logged that it was listening");
        if line.contains("listening") {
            break line;
        }
    };
    let addr = listening_line
        .split_whitespace()
        .find_map(|word| {
            word.strip_prefix("addr=")?
                .parse::<std::net::SocketAddr>()
                .ok()
        })
        .unwrap_or_else(|| panic!("no address in {listening_line:?}"));

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
    let child = server.0.take().unwrap();
    thread::spawn(move || exited_tx.send(child.wait_with_output().unwrap()));
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
