//! The `nama-server` binary.

use std::{
    future::Future,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
};

use clap::Parser;
use nama_plugin_api::{
    auth::Authenticator,
    points::{ServerStarted, ServerStartedEvent},
};
use nama_server::{
    config::{self, Config},
    db::{self, Database},
    plugins::Registry,
};
use tokio::net::TcpListener;
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
        name = %config.name,
        listen = %config.listen,
        data_dir = %config.data_dir.display(),
        "loaded config"
    );

    let Some(plugins) = enable_plugins(&config).await else {
        return ExitCode::FAILURE;
    };

    let db = match Database::open(&config.data_dir).await {
        Ok(db) => db,
        Err(err) => {
            error!(
                "could not open the database in {}: {err}",
                config.data_dir.display()
            );
            return ExitCode::FAILURE;
        }
    };
    info!(
        path = %config.data_dir.join(db::FILE_NAME).display(),
        migrations_applied = ?db.migrations_applied(),
        "opened database"
    );

    let listener = match TcpListener::bind(config.listen).await {
        Ok(listener) => listener,
        Err(err) => {
            error!("could not listen on {}: {err}", config.listen);
            db.close().await;
            return ExitCode::FAILURE;
        }
    };
    // Log the bound address, which differs from `listen` when that asks for port 0.
    match listener.local_addr() {
        Ok(addr) => info!(%addr, "listening"),
        Err(err) => {
            error!("could not read the address listened on: {err}");
            db.close().await;
            return ExitCode::FAILURE;
        }
    }

    plugins.notify::<ServerStarted>(ServerStartedEvent {
        name: config.name.clone(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    });

    let shutdown = async {
        shutdown.await;
        info!("shutting down");
    };
    let db = Arc::new(db);
    let served = nama_server::serve(listener, &config, Arc::clone(&db), plugins, shutdown).await;
    // Serving has finished, so this is the last handle and the database can close cleanly.
    if let Some(db) = Arc::into_inner(db) {
        db.close().await;
    } else {
        error!("the database was still in use at shutdown; it was not closed cleanly");
    }
    if let Err(err) = served {
        error!("server failed: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Enables the Built-in Plugins `config` asks for. Returns `None`, having logged why, if none of
/// them provides an [`Authenticator`], since then nobody could sign in.
async fn enable_plugins(config: &Config) -> Option<Registry> {
    let plugins = Registry::with_builtins(&config.plugins);
    let enabled: Vec<String> = plugins.enabled().iter().map(ToString::to_string).collect();
    info!(?enabled, "enabled plugins");
    let has_authenticator = plugins
        .call_provider(|_: Arc<dyn Authenticator>| async {})
        .await
        .is_some();
    if !has_authenticator {
        error!(
            "no Authenticator is enabled, so nobody could sign in: enable the local-auth plugin \
             (remove `enabled = false` from [plugins.local-auth] in the config), or another \
             plugin that provides one"
        );
        return None;
    }
    Some(plugins)
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
