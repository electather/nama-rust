//! The `nama-server` binary.

use std::{
    env,
    future::Future,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
};

use clap::Parser;
use nama_plugin_api::points::{ServerStarted, ServerStartedEvent};
use nama_server::{
    codes::SetupCode,
    config::{self, Config},
    db::{self, Database},
    ffmpeg::Ffmpeg,
    plugin_settings::PluginSettings,
    plugins::Registry,
    users::Users,
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
    let Some(plugins) = start_plugins(&config, &db).await else {
        close(db).await;
        return ExitCode::FAILURE;
    };
    let setup_code = match setup_code(&db, &plugins).await {
        Ok(setup_code) => setup_code,
        Err(err) => {
            error!("could not prepare the Setup Code: {err}");
            close(db).await;
            return ExitCode::FAILURE;
        }
    };

    let ffmpeg = find_ffmpeg(&config).await;
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

    plugins
        .extension_points()
        .notify::<ServerStarted>(ServerStartedEvent {
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
        ffmpeg,
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

/// Starts the Built-in Plugins and the Installed Plugins in the data directory. Returns `None`,
/// having logged why, if they can't start.
async fn start_plugins(config: &Config, db: &Arc<Database>) -> Option<PluginSettings> {
    PluginSettings::start(Arc::clone(db), Registry::with_builtins(), config)
        .await
        .inspect_err(|err| error!("{err}"))
        .ok()
}

/// A new Setup Code, logged, while the Server has no Users, so whoever can read the log can create
/// the first Admin.
async fn setup_code(
    db: &Arc<Database>,
    plugins: &PluginSettings,
) -> Result<Option<SetupCode>, Box<dyn std::error::Error>> {
    let users = Users::new(Arc::clone(db), plugins.extension_points());
    if users.any().await? {
        return Ok(None);
    }
    let code = SetupCode::generate()?;
    info!(setup_code = %code, "this Server has no Users yet: create the first Admin with this Setup Code");
    Ok(Some(code))
}

/// The ffmpeg and ffprobe at the paths `config` sets or on `PATH`, logging what was found, or why
/// the Server can only Direct Play without them.
async fn find_ffmpeg(config: &Config) -> Ffmpeg {
    let config = config.clone();
    let path = env::var_os("PATH").unwrap_or_default();
    let ffmpeg = task::spawn_blocking(move || Ffmpeg::find(&config, &path))
        .await
        .inspect_err(|err| error!("looking for ffmpeg failed: {err}"))
        .unwrap_or_default();
    if let Some(reason) = ffmpeg.direct_play_only_because() {
        warn!("{reason}, so this Server can only Direct Play, not Remux or Transcode");
    } else {
        info!(?ffmpeg, "found ffmpeg and ffprobe");
    }
    ffmpeg
}

/// Closes the database, which must hold the last handle to it.
async fn close(db: Arc<Database>) {
    if let Some(db) = Arc::into_inner(db) {
        db.close().await;
    } else {
        error!("the database was still in use at shutdown; it was not closed cleanly");
    }
}

/// Logs to stdout at `info`, or at whatever `RUST_LOG` asks for.
fn init_tracing() {
    // Extism logs an Installed Plugin's error or panic as it is, which may quote a secret setting;
    // the Server logs it itself with secrets blanked.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,extism::plugin=off"));
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
