//! A Server serving the API on a random local port, for tests that call it the way clients do.

#[path = "fulfiller.rs"]
pub(crate) mod fulfiller;
#[path = "requests.rs"]
pub(crate) mod requests;
#[path = "tmdb.rs"]
pub(crate) mod tmdb;

use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient, ServerStream},
};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, CreateLibraryRequest, CreateUserRequest,
    EventServiceClient, EventType, ImageSettings, LibraryKind, LibraryServiceClient, Permission,
    PluginServiceClient, Quality, ReplaceQualitiesRequest, ScanLibraryRequest, ServerServiceClient,
    SessionDelivery, SetPermissionsRequest, SetupRequest, SignInRequest, SignInServiceClient,
    SubscribeRequest, SubscribeResponse, UpdateImageSettingsRequest, UserServiceClient, event,
    subscribe_response,
};
use nama_server::{
    codes::SetupCode, config::Config, db::Database, ffmpeg::Ffmpeg,
    plugin_settings::PluginSettings, plugins::Registry,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

use fulfiller::{FakeFulfiller, Fulfills};

/// How long any wait in a test lasts before it fails the test.
pub(crate) const WAIT: Duration = Duration::from_secs(10);

/// The password of every User a test creates.
pub(crate) const PASSWORD: &str = "correct horse battery staple";

/// A Server with its own empty data directory and an Admin signed in, serving until dropped.
/// Server tests build on it: see "Tests" in `crates/nama-server/AGENTS.md`.
pub(crate) struct TestServer {
    pub(crate) addr: SocketAddr,
    /// The session token of the Admin's Device.
    pub(crate) admin_token: String,
    db: Arc<Database>,
    tmdb: Option<FakeTmdb>,
    fulfiller: Option<FakeFulfiller>,
    _stop: oneshot::Sender<()>,
    data_dir: TempDir,
}

/// How a [`TestServer`] starts: its config, Plugins, ffmpeg, and whether it has a fake TMDB and
/// a fake Fulfiller.
#[derive(Default)]
pub(crate) struct Builder {
    config: Config,
    plugins: Registry,
    ffmpeg: Ffmpeg,
    tmdb: bool,
    fulfiller: Option<(Fulfills, FakeFulfiller)>,
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

    fn fake_fulfiller(self, fulfills: Fulfills, profiles: &[&str]) -> Self {
        let fake = FakeFulfiller::default();
        fake.offer_profiles(profiles);
        Self {
            fulfiller: Some((fulfills, fake)),
            ..self
        }
    }

    /// Starts serving on a free port, with a fresh database and the Built-in `local-auth` among
    /// its Plugins, and signs an Admin in.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start(self) -> TestServer {
        self.serve(tempfile::tempdir().unwrap(), None).await
    }

    /// Stops `server` and starts serving its data directory again, as a restart does, with its
    /// Admin still signed in.
    pub(crate) async fn restart(self, server: TestServer) -> TestServer {
        let TestServer {
            admin_token,
            data_dir,
            ..
        } = server;
        self.serve(data_dir, Some(admin_token)).await
    }

    /// Serves `data_dir`, signed in as `admin_token`, or else setting the first Admin up.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn serve(self, data_dir: TempDir, admin_token: Option<String>) -> TestServer {
        let Self {
            config,
            plugins,
            ffmpeg,
            tmdb,
            fulfiller,
        } = self;
        plugins.describe_builtin(|| Arc::new(LocalAuth));
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
            ..config
        };
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
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
            tmdb,
            fulfiller,
            _stop: stop,
            data_dir,
        }
    }
}

/// A User signed in on a Device named Phone. Holds a session token, so it skips `Debug`.
pub(crate) struct SignedIn {
    pub(crate) id: i64,
    pub(crate) token: String,
    client_config: ClientConfig,
}

impl SignedIn {
    /// Client settings that point at the Server and call it as this User.
    pub(crate) fn client_config(&self) -> ClientConfig {
        self.client_config.clone()
    }
}

impl TestServer {
    /// A builder for a Server with the default config, no Plugins but the Built-in `local-auth`,
    /// no ffmpeg, no TMDB and no fake Fulfiller.
    pub(crate) fn builder() -> Builder {
        Builder::default()
    }

    /// The Server's database, for a test that reads or changes what the API can't.
    pub(crate) fn db(&self) -> &Arc<Database> {
        &self.db
    }

    /// The fake TMDB, for a test that reads the requests it got.
    ///
    /// # Panics
    ///
    /// If the builder didn't turn TMDB on.
    #[expect(
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn tmdb(&self) -> &FakeTmdb {
        self.tmdb
            .as_ref()
            .expect("TMDB is on: `TestServer::builder().tmdb()`")
    }

    /// The fake Fulfiller, for a test that changes how it answers or reads what it was handed.
    ///
    /// # Panics
    ///
    /// If the builder didn't turn a Fulfiller on.
    #[expect(
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn fulfiller(&self) -> &FakeFulfiller {
        self.fulfiller
            .as_ref()
            .expect("a Fulfiller is on: `TestServer::builder().fulfiller(..)`")
    }

    /// Replaces the Plugin `plugin`'s Qualities with `qualities` as the Admin, returning them as
    /// saved.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn replace_qualities(
        &self,
        plugin: &str,
        qualities: Vec<Quality>,
    ) -> Vec<Quality> {
        self.try_replace_qualities(plugin, qualities).await.unwrap()
    }

    /// [`Self::replace_qualities`], returning its error, such as a Refusal.
    pub(crate) async fn try_replace_qualities(
        &self,
        plugin: &str,
        qualities: Vec<Quality>,
    ) -> Result<Vec<Quality>, ConnectError> {
        let replaced = self
            .plugin_service()
            .replace_qualities(ReplaceQualitiesRequest {
                id: plugin.to_owned(),
                qualities,
                ..Default::default()
            })
            .await?;
        Ok(replaced.into_owned().qualities)
    }

    /// Sets as the Admin whether images are resized and the most MiB their copies take.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn set_image_settings(
        &self,
        resize_images: bool,
        resized_cache_mib: Option<u32>,
    ) {
        self.try_set_image_settings(resize_images, resized_cache_mib)
            .await
            .unwrap();
    }

    /// [`Self::set_image_settings`], returning its error, such as a Refusal.
    pub(crate) async fn try_set_image_settings(
        &self,
        resize_images: bool,
        resized_cache_mib: Option<u32>,
    ) -> Result<(), ConnectError> {
        let settings = ImageSettings {
            resize_images,
            resized_cache_mib,
            ..Default::default()
        };
        ServerServiceClient::new(HttpClient::plaintext(), self.client_config())
            .update_image_settings(UpdateImageSettingsRequest {
                settings: settings.into(),
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// Stops the fake TMDB, as when TMDB is down.
    pub(crate) fn stop_tmdb(&mut self) {
        self.tmdb = None;
    }

    /// The Server's data directory, such as for its Installed Plugins.
    pub(crate) fn data_dir(&self) -> &Path {
        self.data_dir.path()
    }

    /// Client settings that point at this Server and call it as the Admin.
    pub(crate) fn client_config(&self) -> ClientConfig {
        self.client_config_as(&self.admin_token)
    }

    /// Client settings that point at this Server and present `token` as a bearer token.
    pub(crate) fn client_config_as(&self, token: &str) -> ClientConfig {
        self.anonymous_client_config()
            .with_default_header("authorization", format!("Bearer {token}"))
    }

    /// Client settings that point at this Server and carry no session.
    pub(crate) fn anonymous_client_config(&self) -> ClientConfig {
        anonymous_config(self.addr)
    }

    /// Creates the User `name` as the Admin, gives them exactly `permissions` and signs them in.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn user(&self, name: &str, permissions: &[Permission]) -> SignedIn {
        let id = self.create_user(name).await;
        self.users()
            .set_permissions(SetPermissionsRequest {
                user_id: id,
                permissions: permissions.iter().map(|&p| p.into()).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        self.sign_in(id, name).await
    }

    /// Creates the User `name` as the Admin with the Server's default Permissions and signs them
    /// in.
    pub(crate) async fn user_with_default_permissions(&self, name: &str) -> SignedIn {
        let id = self.create_user(name).await;
        self.sign_in(id, name).await
    }

    /// Creates a Library of `kind` as the Admin, over a fresh folder holding stand-in videos at
    /// `files`, and starts scanning it without waiting for the scan.
    pub(crate) async fn library(&self, kind: LibraryKind, files: &[&str]) -> Library {
        let library = self.create_library(kind, files).await;
        self.scan(library.id).await;
        library
    }

    /// [`Self::library`], returning once Matching has finished.
    pub(crate) async fn matched_library(&self, kind: LibraryKind, files: &[&str]) -> Library {
        let library = self.create_library(kind, files).await;
        self.scan_and_match(&library).await;
        library
    }

    /// [`Self::library`] without the scan, for a test that watches or times the scan itself.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn create_library(&self, kind: LibraryKind, files: &[&str]) -> Library {
        let dir = tempfile::tempdir().unwrap();
        // The Server stores and sends a folder's canonical path, which a temp dir's may not be.
        let folder = dir.path().canonicalize().unwrap();
        let id = self
            .libraries()
            .create_library(CreateLibraryRequest {
                name: "Media".to_owned(),
                kind: kind.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap()
            .id;
        let library = Library {
            id,
            folder,
            _dir: dir,
        };
        library.add(files);
        library
    }

    /// Starts scanning the Library `id` as the Admin, without waiting for the scan.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn scan(&self, id: i64) {
        self.libraries()
            .scan_library(ScanLibraryRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// Scans `library` as the Admin until Matching has finished, and returns the first Item the
    /// scan stored for each file, by the file's path relative to the Library's folder.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn scan_and_match(&self, library: &Library) -> HashMap<String, i64> {
        let events = self
            .scan_until(library.id, |event| {
                matches!(event, event::Event::MatchingFinished(_))
            })
            .await;
        let mut items = HashMap::new();
        for event in events {
            if let event::Event::LibraryItemAdded(added) = event {
                let path = Path::new(&added.path)
                    .strip_prefix(&library.folder)
                    .unwrap();
                let relative = path.to_str().unwrap().to_owned();
                items.entry(relative).or_insert(added.item_id);
            }
        }
        items
    }

    /// Scans the Library `library` until Probing has caught up, which is when Probing has thrown
    /// away the subtitles cached for files gone or changed.
    pub(crate) async fn scan_and_probe(&self, library: &Library) {
        self.scan_until(library.id, |event| {
            matches!(event, event::Event::ProbingFinished(_))
        })
        .await;
    }

    /// Scans the Library `id` as the Admin, and returns the events about it from the scan's
    /// `ScanStarted` on, up to and including the first that `last` picks, waiting as
    /// `ScanLibrary` says to.
    pub(crate) async fn scan_until(
        &self,
        id: i64,
        last: fn(&event::Event) -> bool,
    ) -> Vec<event::Event> {
        let about = SubscribeRequest {
            library_ids: vec![id],
            ..Default::default()
        };
        let mut subscription = subscribe_with(&self.client_config(), about).await;
        self.scan(id).await;
        while !matches!(subscription.next().await, event::Event::ScanStarted(_)) {}
        let mut events = Vec::new();
        loop {
            let next = subscription.next().await;
            let done = last(&next);
            events.push(next);
            if done {
                return events;
            }
        }
    }

    fn libraries(&self) -> LibraryServiceClient<HttpClient> {
        LibraryServiceClient::new(HttpClient::plaintext(), self.client_config())
    }

    fn plugin_service(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.client_config())
    }

    fn users(&self) -> UserServiceClient<HttpClient> {
        UserServiceClient::new(HttpClient::plaintext(), self.client_config())
    }

    /// Creates the User `name` with [`PASSWORD`] as the Admin, and returns their id.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn create_user(&self, name: &str) -> i64 {
        self.users()
            .create_user(CreateUserRequest {
                name: name.to_owned(),
                password: PASSWORD.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .id
    }

    /// Signs the User `id`, named `name`, in on a new Device.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn sign_in(&self, id: i64, name: &str) -> SignedIn {
        let token =
            SignInServiceClient::new(HttpClient::plaintext(), self.anonymous_client_config())
                .sign_in(SignInRequest {
                    name: name.to_owned(),
                    password: PASSWORD.to_owned(),
                    device_name: "Phone".to_owned(),
                    delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
                    ..Default::default()
                })
                .await
                .unwrap()
                .into_owned()
                .token;
        SignedIn {
            id,
            client_config: self.client_config_as(&token),
            token,
        }
    }
}

/// A Library a test made, over a folder of its own that lives as long as the Library does.
pub(crate) struct Library {
    pub(crate) id: i64,
    folder: PathBuf,
    _dir: TempDir,
}

impl Library {
    /// The Library's folder, as the Server stores it.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// The absolute path of the file at `relative` in the Library's folder, as the Server sends it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn path(&self, relative: &str) -> String {
        self.folder.join(relative).to_str().unwrap().to_owned()
    }

    /// Writes stand-in videos at `files`, relative to the Library's folder.
    pub(crate) fn add(&self, files: &[&str]) {
        for file in files {
            self.write(file, b"video");
        }
    }

    /// Writes `contents` to the file at `relative`, creating the folders it is in.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.folder.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

/// Client settings that point at the Server on `addr` and carry no session.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn anonymous_config(addr: SocketAddr) -> ClientConfig {
    ClientConfig::new(format!("http://{addr}").parse().unwrap())
}

/// Creates the first Admin with `setup_code` and returns their Device's session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_up(config: ClientConfig, setup_code: &SetupCode) -> String {
    SignInServiceClient::new(HttpClient::plaintext(), config)
        .setup(SetupRequest {
            setup_code: setup_code.to_string(),
            name: "Admin".to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Test".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token
}

/// Subscribes to events of `types`, or of every type if none, as whoever `who` calls the Server
/// as: the Admin, a [`SignedIn`] User, any other session, or no one.
pub(crate) async fn subscribe(who: &ClientConfig, types: &[EventType]) -> Subscription {
    let only = SubscribeRequest {
        types: types.iter().map(|&kind| kind.into()).collect(),
        ..Default::default()
    };
    subscribe_with(who, only).await
}

/// Subscribes with `request`'s filters as whoever `who` calls the Server as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn subscribe_with(who: &ClientConfig, request: SubscribeRequest) -> Subscription {
    let stream = EventServiceClient::new(HttpClient::plaintext(), who.clone())
        .subscribe(request)
        .await
        .unwrap();
    Subscription(stream)
}

/// An open `Subscribe` stream, read one message at a time within [`WAIT`].
pub(crate) struct Subscription(ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>);

impl Subscription {
    /// The next Event, failing the test on a lag, an error, the stream's end or a timeout.
    #[expect(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn next(&mut self) -> event::Event {
        let next = self
            .message()
            .await
            .unwrap()
            .expect("the stream ended")
            .message;
        let Some(subscribe_response::Message::Event(event)) = next else {
            panic!("not an event: {next:?}");
        };
        event.event.expect("an event of no kind")
    }

    /// The next message as it came, `None` once the stream ends, failing the test on a timeout.
    #[expect(
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn message(&mut self) -> Result<Option<SubscribeResponse>, ConnectError> {
        let message = tokio::time::timeout(WAIT, self.0.message::<SubscribeResponse>())
            .await
            .expect("timed out waiting for the stream")?;
        Ok(message.map(|message| message.to_owned_message()))
    }
}
