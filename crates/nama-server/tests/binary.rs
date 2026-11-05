//! Runs the real `nama-server` binary.
#![cfg(unix)]

use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn server_logs_its_config_and_exits_cleanly_on_ctrl_c() {
    let dir = std::env::temp_dir().join(format!("nama-server-binary-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_file = dir.join("server.toml");
    std::fs::write(&config_file, "data_dir = \"/srv/from-file\"\n").unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_nama-server"))
        .arg("--config")
        .arg(&config_file)
        .env("NAMA_LISTEN", "127.0.0.1:7123")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

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
    assert!(config_line.contains("127.0.0.1:7123"), "{config_line}");
    assert!(config_line.contains("/srv/from-file"), "{config_line}");

    let kill = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(kill.success());

    let (status_tx, status) = mpsc::channel();
    thread::spawn(move || status_tx.send(child.wait_with_output().unwrap()));
    let output = status
        .recv_timeout(TIMEOUT)
        .expect("server did not exit after Ctrl-C");
    assert!(
        output.status.success(),
        "server exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
