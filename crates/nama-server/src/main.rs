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
    plugin_settings,
    plugins::{Registry, installed},
    users::{SetupCode, Users},
};
use tokio::{net::TcpListener, task};
use tracing::{error, info, warn};
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

    let Some(db) = open_database(&config).await else {
        return ExitCode::FAILURE;
    };
    let Some(plugins) = enable_plugins(&config, &db).await else {
        close(db).await;
        return ExitCode::FAILURE;
    };
    log_installed_plugins(&config).await;
    let setup_code = match setup_code(Users::new(Arc::clone(&db), plugins.clone())).await {
        Ok(setup_code) => setup_code,
        Err(err) => {
            error!("could not prepare the Setup Code: {err}");
            close(db).await;
            return ExitCode::FAILURE;
        }
    };

    let listener = match TcpListener::bind(config.listen).await {
        Ok(listener) => listener,
        Err(err) => {
            error!("could not listen on {}: {err}", config.listen);
            close(db).await;
            return ExitCode::FAILURE;
        }
    };
    // Log the bound address, which differs from `listen` when that asks for port 0.
    match listener.local_addr() {
        Ok(addr) => info!(%addr, "listening"),
        Err(err) => {
            error!("could not read the address listened on: {err}");
            close(db).await;
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
    let served = nama_server::serve(
        listener,
        &config,
        Arc::clone(&db),
        plugins,
        setup_code,
        shutdown,
    )
    .await;
    // Serving has finished, so this is the last handle and the database can close cleanly.
    close(db).await;
    if let Err(err) = served {
        error!("server failed: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Opens the database in the data directory. Returns `None`, having logged why, if it can't.
async fn open_database(config: &Config) -> Option<Arc<Database>> {
    let db = match Database::open(&config.data_dir).await {
        Ok(db) => db,
        Err(err) => {
            error!(
                "could not open the database in {}: {err}",
                config.data_dir.display()
            );
            return None;
        }
    };
    info!(
        path = %config.data_dir.join(db::FILE_NAME).display(),
        migrations_applied = ?db.migrations_applied(),
        "opened database"
    );
    Some(Arc::new(db))
}

/// Logs the Installed Plugins in the data directory, without enabling any.
async fn log_installed_plugins(config: &Config) {
    let data_dir = config.data_dir.clone();
    if let Err(err) = task::spawn_blocking(move || installed::log_installed(&data_dir)).await {
        error!("reading the Installed Plugins failed: {err}");
    }
}

/// A new Setup Code, logged, while the Server has no Users, so whoever can read the log can create
/// the first Admin.
async fn setup_code(users: Users) -> Result<Option<SetupCode>, Box<dyn std::error::Error>> {
    if users.any().await? {
        return Ok(None);
    }
    let code = SetupCode::generate()?;
    info!(setup_code = %code, "this Server has no Users yet: create the first Admin with this Setup Code");
    Ok(Some(code))
}

/// Closes the database, which must hold the last handle to it.
async fn close(db: Arc<Database>) {
    if let Some(db) = Arc::into_inner(db) {
        db.close().await;
    } else {
        error!("the database was still in use at shutdown; it was not closed cleanly");
    }
}

/// Enables the Built-in Plugins with the settings stored in `db`. On the first start there are none
/// yet, so it uses the config's `[plugins.*]` tables and imports them.
/// Returns `None`, having logged why, if the settings can't be read or imported, or no Plugin
/// provides an [`Authenticator`], since then nobody could sign in.
async fn enable_plugins(config: &Config, db: &Database) -> Option<Registry> {
    let stored = plugin_settings::stored(db)
        .await
        .inspect_err(|err| error!("could not read the plugin settings: {err}"))
        .ok()?;
    let in_config: Vec<&str> = config.plugins.ids().collect();
    let importing = stored.is_none();
    let settings = match stored {
        Some(stored) => {
            if !in_config.is_empty() {
                warn!(
                    ignored = ?in_config,
                    "ignoring the [plugins.*] tables in the config: plugin settings live in the \
                     database since they were imported"
                );
            }
            stored
        }
        None => config
            .plugins
            .parse()
            .inspect_err(|err| error!("could not load config: {err}"))
            .ok()?,
    };
    let plugins = Registry::with_builtins(&settings);
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
    // Only once the Server can start, so a config it refuses can still be fixed in the file.
    if importing {
        plugin_settings::import(db, &settings)
            .await
            .inspect_err(|err| error!("could not import the plugin settings: {err}"))
            .ok()?;
        info!(imported = ?in_config, "imported plugin settings from the config");
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

#[cfg(windows)]
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
    Ok(async move {
        ctrl_c.recv().await;
    })
}
