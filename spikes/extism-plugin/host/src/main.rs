//! Runs the guest through each question of issue #11 and prints what happened.
//!
//! Usage: `host <guest-v1.wasm> <guest-v2.wasm>` (`run.sh` builds both and calls this).

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use anyhow::{Context, Result, bail, ensure};
use extism::{Manifest, Plugin, PluginBuilder, Wasm};
use shared::Buffa;
use shared::spike::v1::{FetchRequest, FetchResponse, GreetRequest, GreetResponse};

const SONARR_REPLY: &str = r#"[{"title":"Severance","tvdbId":371980}]"#;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let (Some(v1), Some(v2)) = (args.next(), args.next()) else {
        bail!("usage: host <guest-v1.wasm> <guest-v2.wasm>");
    };
    let (v1, v2) = (Path::new(&v1), Path::new(&v2));

    round_trip(v1)?;
    http_allowlist(v1)?;
    reload(v1, v2)?;
    timeout_and_crash(v1)?;
    call_cost(v1)?;
    Ok(())
}

/// Q1: a buffa request in, a buffa response out.
fn round_trip(wasm: &Path) -> Result<()> {
    let mut plugin = load(Manifest::new([Wasm::file(wasm)]))?;
    let reply = greet(&mut plugin, "Nama")?;
    ensure!(
        reply.message == "Hello, Nama!",
        "unexpected reply {reply:?}"
    );
    println!("Q1 round trip: {reply:?}");
    Ok(())
}

/// Q2: the guest reaches an allowed host and is refused any other.
fn http_allowlist(wasm: &Path) -> Result<()> {
    let sonarr = serve_mock_sonarr()?;
    let manifest = Manifest::new([Wasm::file(wasm)]).with_allowed_host("127.0.0.1");
    let mut plugin = load(manifest)?;

    let allowed = fetch(&mut plugin, &format!("http://{sonarr}/api/v3/series"))?;
    ensure!(
        allowed.status == 200 && allowed.body == SONARR_REPLY,
        "{allowed:?}"
    );
    println!(
        "Q2 allowed host 127.0.0.1: status {}, body {}",
        allowed.status, allowed.body
    );

    // Same server, other name: the allowlist matches host names, so this must be refused.
    let refused = fetch(&mut plugin, &format!("http://localhost:{}/", sonarr.port()));
    let error = refused
        .err()
        .context("a host outside the allowlist was reached")?;
    println!("Q2 host localhost refused: {error:#}");

    // A manifest without allowed hosts denies every request rather than allowing all.
    let mut no_allowlist = load(Manifest::new([Wasm::file(wasm)]))?;
    let error = fetch(&mut no_allowlist, &format!("http://{sonarr}/"))
        .err()
        .context("a plugin with no allowed hosts made an HTTP call")?;
    println!("Q2 no allowed hosts at all: {error:#}");
    Ok(())
}

/// Q3: load, call and drop many builds from disk in one process, watching resident memory.
fn reload(v1: &Path, v2: &Path) -> Result<()> {
    const ROUNDS: u32 = 2_000;
    let mut compile_time = Duration::ZERO;
    let mut rss_checkpoints = Vec::new();
    for round in 1..=ROUNDS {
        let (wasm, version) = if round % 2 == 0 {
            (v2, "v2")
        } else {
            (v1, "v1")
        };
        // No compilation cache, so every round really compiles and frees a module.
        let builder = PluginBuilder::new(Manifest::new([Wasm::data(fs::read(wasm)?)]));
        let started = Instant::now();
        let mut plugin = builder.with_cache_disabled().build()?;
        compile_time += started.elapsed();
        let reply = greet(&mut plugin, "reload")?;
        ensure!(
            reply.guest_version == version,
            "loaded {version}, got {reply:?}"
        );
        drop(plugin);
        if [50, 500, 1_000, ROUNDS].contains(&round) {
            rss_checkpoints.push(format!("{round}: {} KiB", rss_kib()?));
        }
    }
    println!(
        "Q3 {ROUNDS} load/call/drop rounds alternating v1/v2, uncached: {:.1} ms per load; RSS after round {}",
        compile_time.as_secs_f64() * 1e3 / f64::from(ROUNDS),
        rss_checkpoints.join(", ")
    );

    let started = Instant::now();
    load(Manifest::new([Wasm::file(v1)]))?;
    println!(
        "Q3 load with wasmtime's default on-disk cache warm: {:?}",
        started.elapsed()
    );

    // Compiling once and instantiating per use is how a Server would hold an Installed Plugin.
    let compiled = PluginBuilder::new(Manifest::new([Wasm::file(v1)])).compile()?;
    const INSTANCES: u32 = 50_000;
    let mut rss_checkpoints = Vec::new();
    let started = Instant::now();
    for round in 1..=INSTANCES {
        let mut plugin = Plugin::new_from_compiled(&compiled)?;
        greet(&mut plugin, "instance")?;
        if [1_000, 10_000, INSTANCES].contains(&round) {
            rss_checkpoints.push(format!("{round}: {} KiB", rss_kib()?));
        }
    }
    println!(
        "Q3 instantiate from a CompiledPlugin + one call + drop: {:.0} µs; RSS after round {}",
        started.elapsed().as_secs_f64() * 1e6 / f64::from(INSTANCES),
        rss_checkpoints.join(", ")
    );
    Ok(())
}

/// Q4: an endless guest is stopped by the timeout, a panicking one traps, and the host carries on.
fn timeout_and_crash(wasm: &Path) -> Result<()> {
    let timeout = Duration::from_millis(100);
    let mut plugin = load(Manifest::new([Wasm::file(wasm)]).with_timeout(timeout))?;

    let started = Instant::now();
    let error = plugin
        .call::<(), ()>("spin", ())
        .err()
        .context("spin returned")?;
    println!(
        "Q4 endless loop with a {timeout:?} timeout: stopped after {:?}: {error:#}",
        started.elapsed()
    );
    println!(
        "Q4 same instance after the timeout: {:?}",
        greet(&mut plugin, "after timeout")
    );

    let error = plugin
        .call::<(), ()>("crash", ())
        .err()
        .context("crash returned")?;
    println!("Q4 panicking guest: {error:#}");
    println!(
        "Q4 same instance after the panic: {:?}",
        greet(&mut plugin, "after panic")
    );
    Ok(())
}

/// Q5: the cost of one call with a small message, once the plugin is loaded.
fn call_cost(wasm: &Path) -> Result<()> {
    const CALLS: u32 = 20_000;
    for (label, manifest) in [
        ("no timeout", Manifest::new([Wasm::file(wasm)])),
        (
            "1 s timeout",
            Manifest::new([Wasm::file(wasm)]).with_timeout(Duration::from_secs(1)),
        ),
    ] {
        let mut plugin = load(manifest)?;
        for _ in 0..1_000 {
            greet(&mut plugin, "warm-up")?;
        }
        let started = Instant::now();
        for _ in 0..CALLS {
            greet(&mut plugin, "bench")?;
        }
        println!(
            "Q5 greet call ({label}): {:.1} µs",
            started.elapsed().as_secs_f64() * 1e6 / f64::from(CALLS)
        );
    }
    Ok(())
}

fn load(manifest: Manifest) -> Result<Plugin> {
    Plugin::new(manifest, [], false)
}

fn greet(plugin: &mut Plugin, name: &str) -> Result<GreetResponse> {
    let request = GreetRequest {
        name: name.into(),
        ..Default::default()
    };
    let Buffa(reply) = plugin.call::<_, Buffa<GreetResponse>>("greet", Buffa(request))?;
    Ok(reply)
}

fn fetch(plugin: &mut Plugin, url: &str) -> Result<FetchResponse> {
    let request = FetchRequest {
        url: url.into(),
        ..Default::default()
    };
    let Buffa(reply) = plugin.call::<_, Buffa<FetchResponse>>("fetch", Buffa(request))?;
    Ok(reply)
}

/// A stand-in for Sonarr: answers every request on a local port with the same JSON.
fn serve_mock_sonarr() -> Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            // ponytail: a failed reply surfaces as a failed fetch in the guest; nothing to do here.
            let _ignored: std::io::Result<()> = answer(stream);
        }
    });
    Ok(address)
}

fn answer(mut stream: TcpStream) -> std::io::Result<()> {
    // Read the request head before replying, or the client may see a reset.
    let mut lines = BufReader::new(&stream).lines();
    while lines
        .next()
        .transpose()?
        .is_some_and(|line| !line.is_empty())
    {}
    write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{SONARR_REPLY}",
        SONARR_REPLY.len()
    )
}

/// Resident memory of this process, from `/proc` (Linux only, which is fine for a spike).
fn rss_kib() -> Result<u64> {
    let status = fs::read_to_string("/proc/self/status")?;
    let line = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .context("no VmRSS")?;
    Ok(line
        .split_whitespace()
        .nth(1)
        .context("malformed VmRSS")?
        .parse()?)
}
