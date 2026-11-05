//! The `nama-server` binary.

use std::{
    future::Future,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
};

use clap::Parser;
use nama_server::config::{self, Config};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// TOML config file. Defaults to `nama.toml` in the working directory, if it exists.
    #[arg(long, env = config::FILE_ENV)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    init_tracing();

    let config = match Config::load(args.config.as_deref()) {
        Ok(config) => config,
        Err(err) => {
            error!("could not load config: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Listen before announcing startup, so a Ctrl-C right after the log line isn't missed.
    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(err) => {
            error!("could not listen for shutdown signals: {err}");
            return ExitCode::FAILURE;
        }
    };

    info!(
        listen = %config.listen,
        data_dir = %config.data_dir.display(),
        "loaded config"
    );

    shutdown.await;
    info!("shutting down");
    ExitCode::SUCCESS
}

/// Logs to stdout at `info`, or at whatever `RUST_LOG` asks for.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(io::stdout().is_terminal())
        .init();
}

/// Resolves on Ctrl-C (SIGINT), or SIGTERM on Unix.
#[cfg(unix)]
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    })
}

/// Resolves on Ctrl-C.
#[cfg(windows)]
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
    Ok(async move {
        ctrl_c.recv().await;
    })
}
