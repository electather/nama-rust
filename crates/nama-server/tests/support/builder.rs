//! How a [`TestServer`] starts, and starting it.

use std::sync::Arc;

use nama_plugin_local_auth::LocalAuth;
use nama_plugin_tmdb::Tmdb;
use nama_server::{
    config::Config, db::Database, ffmpeg::Ffmpeg, plugin_settings::PluginSettings,
    plugins::Registry, users::SetupCode,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

use super::{
    CLOCK, ClockGuard, TestServer, anonymous_config,
    fulfiller::{self, FakeFulfiller, Fulfills},
    set_up, tmdb,
};

/// How a [`TestServer`] starts: its config, Plugins, ffmpeg, and whether it has a fake TMDB and
/// a fake Fulfiller.
#[derive(Default)]
pub(crate) struct Builder {
    config: Config,
    plugins: Registry,
    ffmpeg: Ffmpeg,
    tmdb: bool,
    fulfiller: Option<(Fulfills, FakeFulfiller)>,
    moves_clock: bool,
}

impl Builder {
    /// Serves `config`, with the data directory replaced by a fresh one.
    pub(crate) fn config(self, config: Config) -> Self {
        Self { config, ..self }
    }

    /// Starts the Plugins `plugins` knows as a Server starts them, keeping those it already
    /// enabled.
    pub(crate) fn plugins(self, plugins: Registry) -> Self {
        Self { plugins, ..self }
    }

    /// Has the Server start with `ffmpeg` as what it found at startup.
    pub(crate) fn ffmpeg(self, ffmpeg: Ffmpeg) -> Self {
        Self { ffmpeg, ..self }
    }

    /// Has the Server start with a fake TMDB and the TMDB Plugin enabled against it, for metadata.
    pub(crate) fn tmdb(self) -> Self {
        Self { tmdb: true, ..self }
    }

    /// Has the Server start with a fake Movie Fulfiller enabled, as the Built-in Plugin
    /// [`fulfiller::MOVIE_FULFILLER`], whose settings check offers the service profiles
    /// `profiles`.
    pub(crate) fn fulfiller(self, profiles: &[&str]) -> Self {
        self.fake_fulfiller(Fulfills::Movies, profiles)
    }

    /// [`Self::fulfiller`] for Shows, as [`fulfiller::SHOWS_FULFILLER`].
    pub(crate) fn shows_fulfiller(self, profiles: &[&str]) -> Self {
        self.fake_fulfiller(Fulfills::Shows, profiles)
    }

    /// Has the Server hold the clock alone, so its test may call [`TestServer::advance_clock`].
    pub(crate) fn moves_clock(self) -> Self {
        Self {
            moves_clock: true,
            ..self
        }
    }

    fn fake_fulfiller(self, fulfills: Fulfills, profiles: &[&str]) -> Self {
        let fake = FakeFulfiller::default();
        fake.offer_profiles(profiles);
        Self {
            fulfiller: Some((fulfills, fake)),
            ..self
        }
    }

    /// Starts serving on a free port, with a fresh database and the Built-ins `local-auth` and TMDB
    /// among its Plugins, and signs an Admin in.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start(self) -> TestServer {
        let clock = if self.moves_clock {
            let guard = CLOCK.write().await;
            nama_server::db::reset_clock();
            ClockGuard::Exclusive(guard)
        } else {
            ClockGuard::Shared(CLOCK.read().await)
        };
        self.serve(tempfile::tempdir().unwrap(), None, clock).await
    }

    /// Stops `server` and starts serving its data directory again, as a restart does, with its
    /// Admin still signed in. The new Server keeps the old one's hold on the clock, since locking
    /// it again while the test holds it would deadlock.
    pub(crate) async fn restart(self, server: TestServer) -> TestServer {
        let TestServer {
            admin_token,
            data_dir,
            clock,
            ..
        } = server;
        self.serve(data_dir, Some(admin_token), clock).await
    }

    /// Serves `data_dir`, signed in as `admin_token`, or else setting the first Admin up.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn serve(
        self,
        data_dir: TempDir,
        admin_token: Option<String>,
        clock: ClockGuard,
    ) -> TestServer {
        let Self {
            config,
            plugins,
            ffmpeg,
            tmdb,
            fulfiller,
            ..
        } = self;
        plugins.describe_builtin(|| Arc::new(LocalAuth));
        // Every Server has TMDB Built in, and a Library created naming no title source orders it.
        if plugins.plugin(&Tmdb::ID).is_none() {
            plugins.describe_builtin(|| Arc::new(Tmdb::new()));
        }
        let fulfiller = fulfiller.map(|(fulfills, fake)| {
            fulfiller::enable(&plugins, fulfills, &fake);
            fake
        });
        let tmdb = if tmdb {
            Some(tmdb::enable(&plugins).await)
        } else {
            None
        };
        let config = Config {
            data_dir: data_dir.path().to_owned(),
            // macOS's fseventsd slows each Library change to a minute; the scanner's tests watch.
            watch_folders: false,
            acme_directory: "https://127.0.0.1:1/".to_owned(), // So no test reaches a real CA.
            ..config
        };
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let registry = plugins.clone();
        let plugins = PluginSettings::start(Arc::clone(&db), plugins, &config)
            .await
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let setup_code = (admin_token.is_none()).then(|| SetupCode::generate().unwrap());
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(nama_server::serve(
            listener,
            &config,
            Arc::clone(&db),
            plugins,
            setup_code.clone(),
            ffmpeg,
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        let admin_token = match admin_token {
            Some(token) => token,
            None => set_up(anonymous_config(addr), &setup_code.unwrap()).await,
        };
        TestServer {
            addr,
            admin_token,
            db,
            plugins: registry,
            tmdb,
            fulfiller,
            _stop: stop,
            data_dir,
            clock,
        }
    }
}
