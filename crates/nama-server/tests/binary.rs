//! Runs the real `nama-server` binary.
#![cfg(unix)]

#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
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

/// The server's log lines, read on a thread so a hung server fails the test instead of blocking it.
#[expect(
    clippy::unwrap_used,
    reason = "a server whose log can't be read fails the test that reads it"
)]
fn log_lines(server: &mut Child) -> mpsc::Receiver<String> {
    let stdout = BufReader::new(server.stdout.take().unwrap());
    let (lines_tx, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in stdout.lines().map_while(Result::ok) {
            let _: Result<(), mpsc::SendError<String>> = lines_tx.send(line);
        }
    });
    lines
}

/// Starts the server with `config_file`, listening on a free port and logging at `info`.
#[expect(
    clippy::unwrap_used,
    reason = "a server that can't be started fails the test that starts it"
)]
fn spawn_server(config_file: &Path) -> KillOnDrop {
    KillOnDrop(Some(
        Command::new(env!("CARGO_BIN_EXE_nama-server"))
            .arg("--config")
            .arg(config_file)
            .env("NAMA_LISTEN", "127.0.0.1:0")
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ))
}

#[test]
fn server_logs_its_config_creates_its_database_serves_the_api_and_exits_cleanly_on_ctrl_c() {
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

    let mut server = spawn_server(&config_file);
    let child = server.0.as_mut().unwrap();

    let lines = log_lines(child);

    let config_line = wait_for_line(&lines, "loaded config");
    assert!(config_line.contains("127.0.0.1:0"), "{config_line}");
    assert!(
        config_line.contains(&data_dir.display().to_string()),
        "{config_line}"
    );

    // A fresh data directory gets a new database with every migration applied.
    let database_line = wait_for_line(&lines, "opened database");
    assert!(
        database_line.contains(
            "migrations_applied=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]"
        ),
        "{database_line}"
    );
    assert!(data_dir.join("nama.db").is_file());

    // A Server with no Users yet logs the Setup Code that creates the first Admin.
    let setup_line = wait_for_line(&lines, "Setup Code");
    assert!(setup_line.contains("setup_code="), "{setup_line}");

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
    assert!(response.contains(r#""setupRequired":true"#), "{response}");

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
    // The database closed cleanly, checkpointing its WAL, once serving had stopped.
    assert!(!data_dir.join("nama.db-wal").exists());
}

#[test]
fn server_logs_installed_plugins_enables_the_stored_ones_and_skips_the_ones_it_refuses() {
    let dir = std::env::temp_dir().join(format!("nama-server-installed-{}", std::process::id()));
    let plugins_dir = dir.join("data").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    let dir = RemoveOnDrop(dir);
    let data_dir = dir.0.join("data");
    let config_file = dir.0.join("server.toml");
    // Settings that enable the test Plugin, as a restart finds them stored.
    std::fs::write(
        &config_file,
        format!(
            "data_dir = \"{}\"\n\n[plugins.test-plugin]\n",
            data_dir.display()
        ),
    )
    .unwrap();
    std::fs::write(plugins_dir.join("test.wasm"), test_plugin::build("")).unwrap();
    std::fs::write(
        plugins_dir.join("out-of-range.wasm"),
        test_plugin::build("out-of-range"),
    )
    .unwrap();
    std::fs::write(plugins_dir.join("panics.wasm"), test_plugin::build("panic")).unwrap();
    std::fs::write(plugins_dir.join("not-wasm.wasm"), "not a WASM module").unwrap();
    // WASM's magic number and version, then a byte that starts no valid section.
    std::fs::write(plugins_dir.join("corrupt.wasm"), b"\0asm\x01\0\0\0\xff").unwrap();
    // The smallest valid WASM module: the magic number and version, and no exports at all.
    std::fs::write(plugins_dir.join("no-manifest.wasm"), b"\0asm\x01\0\0\0").unwrap();

    let mut server = spawn_server(&config_file);
    let lines = log_lines(server.0.as_mut().unwrap());

    // The Server reads the files in name order.
    let corrupt = wait_for_line(&lines, "corrupt.wasm");
    assert!(corrupt.contains("could not load it"), "{corrupt}");
    let no_manifest = wait_for_line(&lines, "no-manifest.wasm");
    assert!(no_manifest.contains("manifest()"), "{no_manifest}");
    let not_wasm = wait_for_line(&lines, "not-wasm.wasm");
    assert!(not_wasm.contains("not a WASM module"), "{not_wasm}");
    let out_of_range = wait_for_line(&lines, "out-of-range.wasm");
    assert!(
        out_of_range.contains("it wants nama-api <0.1, and this Server has 0.1.0"),
        "{out_of_range}"
    );
    // nama-pdk's panic hook hands the panic's message to the Server.
    let panics = wait_for_line(&lines, "panics.wasm");
    assert!(
        panics.contains("its manifest() export failed: panicked at ")
            && panics.contains(" the test Plugin was built to panic"),
        "{panics}"
    );
    let found = wait_for_line(&lines, "test.wasm");
    assert!(found.contains("id=test-plugin"), "{found}");
    assert!(found.contains("version=0.1.0"), "{found}");
    let enabled = wait_for_line(&lines, "enabled plugins");
    assert!(enabled.contains("\"test-plugin\""), "{enabled}");

    // The refused files don't stop the Server.
    wait_for_line(&lines, "listening");
    let cache = std::fs::read_dir(data_dir.join("wasmtime").join("cache")).unwrap();
    assert!(
        cache.count() > 0,
        "wasmtime cached nothing under the data directory"
    );
}

#[test]
fn server_refuses_to_start_without_an_authenticator_says_why_and_starts_once_the_file_is_fixed() {
    let dir = std::env::temp_dir().join(format!("nama-server-no-auth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = RemoveOnDrop(dir);
    let config_file = dir.0.join("server.toml");
    let data_dir = dir.0.join("data");
    std::fs::write(
        &config_file,
        format!(
            "data_dir = \"{}\"\n\n[plugins.local-auth]\nenabled = false\n",
            data_dir.display()
        ),
    )
    .unwrap();

    let mut server = spawn_server(&config_file);
    let child = server.0.as_mut().unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < TIMEOUT,
            "server started without an Authenticator"
        );
        thread::sleep(Duration::from_millis(50));
    };
    let mut log = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut log)
        .unwrap();

    assert!(!status.success());
    assert!(log.contains("no Authenticator is enabled"), "{log}");

    // The refused start imported nothing, so fixing the file still counts.
    std::fs::write(
        &config_file,
        format!("data_dir = \"{}\"\n", data_dir.display()),
    )
    .unwrap();
    let fixed = log_until_listening(&config_file);
    assert!(fixed.contains("imported plugin settings"), "{fixed}");
}

#[test]
fn plugin_settings_are_imported_from_the_config_once_and_the_config_is_ignored_after() {
    const API_KEY: &str = "tmdb-key-from-the-config";
    let dir = std::env::temp_dir().join(format!("nama-server-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = RemoveOnDrop(dir);
    let config_file = dir.0.join("server.toml");
    let data_dir = format!("data_dir = \"{}\"\n", dir.0.join("data").display());

    std::fs::write(
        &config_file,
        format!(
            "{data_dir}[plugins.tmdb]\napi_key = \"{API_KEY}\"\n\n\
             [plugins.log-events]\nenabled = false\n"
        ),
    )
    .unwrap();
    let first_start = log_until_listening(&config_file);

    std::fs::write(
        &config_file,
        // A value no Plugin setting takes, which an imported table would be refused for.
        format!("{data_dir}[plugins.tmdb]\nenabled = false\nretries = 1.5\n"),
    )
    .unwrap();
    let later_start = log_until_listening(&config_file);

    for log in [&first_start, &later_start] {
        let enabled = log
            .lines()
            .find(|line| line.contains("enabled plugins"))
            .unwrap();
        assert!(enabled.contains(r#"["local-auth", "tmdb"]"#), "{enabled}");
        assert!(!log.contains(API_KEY), "a setting value was logged: {log}");
    }
    assert!(
        first_start.contains("imported plugin settings"),
        "{first_start}"
    );
    assert!(!first_start.contains("ignoring"), "{first_start}");
    let ignoring = later_start
        .lines()
        .find(|line| line.contains("ignoring the [plugins.*] tables"))
        .unwrap();
    assert!(ignoring.contains(r#"["tmdb"]"#), "{ignoring}");
}

/// Starts the server with `config_file`, and returns what it logged up to "listening" once it has
/// stopped it.
#[expect(
    clippy::unwrap_used,
    reason = "a server that can't be started fails the test that starts it"
)]
fn log_until_listening(config_file: &Path) -> String {
    let mut server = spawn_server(config_file);
    let lines = log_lines(server.0.as_mut().unwrap());
    let mut log = String::new();
    loop {
        let line = lines.recv_timeout(TIMEOUT).unwrap();
        log.push_str(&line);
        log.push('\n');
        if line.contains("listening") {
            return log;
        }
    }
}
