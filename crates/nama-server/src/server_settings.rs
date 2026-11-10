//! What an admin sets about the Server as a whole, kept in the database: so far its URL, which
//! notifications link to the web app with, the folder playback writes segment files to, the
//! most Transcodes it runs at once, what they run on, and the default Playback Limit for remote
//! playback.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, PoisonError, RwLock,
        atomic::{AtomicU32, Ordering},
    },
};

use nama_plugin_api::settings::{SettingKind, SettingValue};

use crate::{db::Database, ffmpeg::Accel, playback::PlaybackLimit};

/// The address people reach the Server at, or `""` for none, held in memory so that building a
/// notification doesn't read the database. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerUrl(Arc<RwLock<String>>);

impl ServerUrl {
    /// Whether `url` may be the Server URL: empty, or an http or https URL as a URL setting takes,
    /// with no query or fragment, since links are added to its end.
    pub(crate) fn accepts(url: &str) -> bool {
        url.is_empty()
            || (SettingValue::Text(url.to_owned()).is(SettingKind::Url)
                && !url.contains(['?', '#']))
    }

    /// Takes the stored URL, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT url FROM server_settings")
            .fetch_optional(db.reader())
            .await?;
        self.set(stored.unwrap_or_default());
        Ok(())
    }

    /// Stores `url`, which the caller checked [`ServerUrl::accepts`], and uses it from now on.
    pub(crate) async fn save(&self, db: &Database, url: String) -> Result<(), sqlx::Error> {
        let this = self.clone();
        db.write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO server_settings (id, url) VALUES (1, ?)
                     ON CONFLICT (id) DO UPDATE SET url = excluded.url",
                    url
                )
                .execute(&mut **tx)
                .await?;
                // Here on the one writer, not after `write` returns: saves then take effect in the
                // order they're stored, and even if the caller stopped waiting.
                this.set(url);
                Ok::<_, sqlx::Error>(())
            })
        })
        .await
    }

    /// The URL, or `""` for none.
    pub(crate) fn get(&self) -> String {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Where the Request `id` is in the web app, or `None` with no URL set.
    pub(crate) fn request_link(&self, id: i64) -> Option<String> {
        let url = self.get();
        // The URL may name a path the web app is served under, which a link keeps.
        (!url.is_empty()).then(|| format!("{}/requests/{id}", url.trim_end_matches('/')))
    }

    fn set(&self, url: String) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = url;
    }
}

/// Where playback writes segment files: the folder an admin set, or else a default. Held in
/// memory so that starting a plan doesn't read the database. Clones share it.
#[derive(Debug, Clone)]
pub(crate) struct SegmentFolder {
    default: PathBuf,
    /// As the admin set it, or `""` for the default.
    set: Arc<RwLock<String>>,
}

impl SegmentFolder {
    /// Segment files go to `default` until an admin sets a folder.
    pub(crate) fn new(default: PathBuf) -> Self {
        Self {
            default,
            set: Arc::default(),
        }
    }

    /// Whether `folder` may be the segment folder: empty for the default, or an absolute path,
    /// since a relative one would depend on where the Server was started.
    pub(crate) fn accepts(folder: &str) -> bool {
        folder.is_empty() || Path::new(folder).is_absolute()
    }

    /// Takes the stored folder, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT segment_folder FROM server_settings")
            .fetch_optional(db.reader())
            .await?;
        self.set(stored.unwrap_or_default());
        Ok(())
    }

    /// Stores `folder`, which the caller checked [`SegmentFolder::accepts`], and uses it for plans
    /// started from now on.
    pub(crate) async fn save(&self, db: &Database, folder: String) -> Result<(), sqlx::Error> {
        let this = self.clone();
        db.write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO server_settings (id, url, segment_folder) VALUES (1, '', ?)
                     ON CONFLICT (id) DO UPDATE SET segment_folder = excluded.segment_folder",
                    folder
                )
                .execute(&mut **tx)
                .await?;
                // On the one writer, as `ServerUrl::save` does, so saves take effect in order.
                this.set(folder);
                Ok::<_, sqlx::Error>(())
            })
        })
        .await
    }

    /// The folder as the admin set it, or `""` for the default.
    pub(crate) fn get(&self) -> String {
        self.set
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Where segment files go now.
    pub(crate) fn path(&self) -> PathBuf {
        let set = self.get();
        if set.is_empty() {
            self.default.clone()
        } else {
            set.into()
        }
    }

    /// The folders an earlier run may have left segment files in that this one can know of: the
    /// one set, and the default.
    pub(crate) fn leftover_paths(&self) -> [PathBuf; 2] {
        [self.path(), self.default.clone()]
    }

    fn set(&self, folder: String) {
        *self.set.write().unwrap_or_else(PoisonError::into_inner) = folder;
    }
}

/// The most Transcodes the Server runs at once, as an admin set it, held in memory so that
/// planning doesn't read the database. Unset, the default, depends on what they run on
/// (`RunningPlans::transcodes_full`). Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct MaxTranscodes(Arc<AtomicU32>);

impl MaxTranscodes {
    /// Takes the stored cap, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT max_transcodes FROM server_settings")
            .fetch_optional(db.reader())
            .await?
            .flatten();
        // Only `save` writes it, from a `u32` of at least 1.
        self.set(stored.and_then(|stored| u32::try_from(stored).ok()));
        Ok(())
    }

    /// Stores `max`, at least 1 or `None` for the default, and uses it for plans made from now on.
    pub(crate) async fn save(&self, db: &Database, max: Option<u32>) -> Result<(), sqlx::Error> {
        let this = self.clone();
        db.write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO server_settings (id, url, max_transcodes) VALUES (1, '', ?)
                     ON CONFLICT (id) DO UPDATE SET max_transcodes = excluded.max_transcodes",
                    max
                )
                .execute(&mut **tx)
                .await?;
                // On the one writer, as `ServerUrl::save` does, so saves take effect in order.
                this.set(max);
                Ok::<_, sqlx::Error>(())
            })
        })
        .await
    }

    /// The cap as set, or `None` for the default.
    pub(crate) fn get(&self) -> Option<u32> {
        // 0 stands for unset, as a cap is at least 1.
        Some(self.0.load(Ordering::Relaxed)).filter(|&max| max > 0)
    }

    fn set(&self, max: Option<u32>) {
        self.0.store(max.unwrap_or_default(), Ordering::Relaxed);
    }
}

/// What an admin chose for Transcodes to run on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HardwareChoice {
    /// The first API that passed its test at startup, or software without one.
    #[default]
    Automatic,
    /// Software only.
    Off,
    /// This API, if it passed its test at startup, or else software.
    Api(Accel),
}

impl HardwareChoice {
    /// As the database stores it.
    const fn stored(self) -> &'static str {
        match self {
            Self::Automatic => "",
            Self::Off => "off",
            Self::Api(Accel::Nvenc) => "nvenc",
            Self::Api(Accel::Qsv) => "qsv",
            Self::Api(Accel::Vaapi) => "vaapi",
            Self::Api(Accel::VideoToolbox) => "videotoolbox",
        }
    }

    /// The choice the database stores as `stored`, Automatic for one this Server doesn't know.
    fn from_stored(stored: &str) -> Self {
        std::iter::once(Self::Off)
            .chain(Accel::ALL.map(Self::Api))
            .find(|choice| choice.stored() == stored)
            .unwrap_or_default()
    }
}

/// What an admin chose for Transcodes to run on, held in memory so that planning doesn't read the
/// database. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct HardwareAcceleration(Arc<RwLock<HardwareChoice>>);

impl HardwareAcceleration {
    /// Takes the stored choice, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT hardware_acceleration FROM server_settings")
            .fetch_optional(db.reader())
            .await?;
        self.set(
            stored
                .as_deref()
                .map(HardwareChoice::from_stored)
                .unwrap_or_default(),
        );
        Ok(())
    }

    /// Stores `choice` and uses it for plans made from now on.
    pub(crate) async fn save(
        &self,
        db: &Database,
        choice: HardwareChoice,
    ) -> Result<(), sqlx::Error> {
        let this = self.clone();
        db.write(move |tx| {
            Box::pin(async move {
                let stored = choice.stored();
                sqlx::query!(
                    "INSERT INTO server_settings (id, url, hardware_acceleration) VALUES (1, '', ?)
                     ON CONFLICT (id) DO UPDATE
                     SET hardware_acceleration = excluded.hardware_acceleration",
                    stored
                )
                .execute(&mut **tx)
                .await?;
                // On the one writer, as `ServerUrl::save` does, so saves take effect in order.
                this.set(choice);
                Ok::<_, sqlx::Error>(())
            })
        })
        .await
    }

    /// The choice now.
    pub(crate) fn get(&self) -> HardwareChoice {
        *self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, choice: HardwareChoice) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = choice;
    }
}

/// The Playback Limit for a Device outside the local network until its User picks one, `None`
/// being Original. Held in memory so that planning doesn't read the database. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct RemotePlaybackLimit(Arc<RwLock<Option<PlaybackLimit>>>);

impl RemotePlaybackLimit {
    /// Takes the stored limit, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT remote_playback_limit FROM server_settings")
            .fetch_optional(db.reader())
            .await?;
        self.set(stored.as_deref().and_then(PlaybackLimit::named));
        Ok(())
    }

    /// Stores `limit` and uses it for plans made from now on.
    pub(crate) async fn save(
        &self,
        db: &Database,
        limit: Option<PlaybackLimit>,
    ) -> Result<(), sqlx::Error> {
        let this = self.clone();
        db.write(move |tx| {
            Box::pin(async move {
                let name = limit.map_or("", PlaybackLimit::name);
                sqlx::query!(
                    "INSERT INTO server_settings (id, url, remote_playback_limit) VALUES (1, '', ?)
                     ON CONFLICT (id) DO UPDATE
                     SET remote_playback_limit = excluded.remote_playback_limit",
                    name
                )
                .execute(&mut **tx)
                .await?;
                // On the one writer, as `ServerUrl::save` does, so saves take effect in order.
                this.set(limit);
                Ok::<_, sqlx::Error>(())
            })
        })
        .await
    }

    /// The limit, `None` being Original.
    pub(crate) fn get(&self) -> Option<PlaybackLimit> {
        *self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, limit: Option<PlaybackLimit>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = limit;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HardwareAcceleration, HardwareChoice, MaxTranscodes, RemotePlaybackLimit, SegmentFolder,
        ServerUrl,
    };
    use crate::{db::Database, ffmpeg::Accel, playback::PlaybackLimit};

    #[tokio::test]
    async fn a_saved_url_is_loaded_again_after_a_restart() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        ServerUrl::default()
            .save(&db, "https://nama.example.com".to_owned())
            .await
            .unwrap();

        let restarted = ServerUrl::default();
        restarted.load(&db).await.unwrap();

        assert_eq!(restarted.get(), "https://nama.example.com");
    }

    #[tokio::test]
    async fn a_saved_segment_folder_is_loaded_again_after_a_restart_and_keeps_the_server_url() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let url = ServerUrl::default();
        url.save(&db, "https://nama.example.com".to_owned())
            .await
            .unwrap();
        SegmentFolder::new("/data/transcodes".into())
            .save(&db, "/tmp/nama".to_owned())
            .await
            .unwrap();

        let restarted = SegmentFolder::new("/data/transcodes".into());
        restarted.load(&db).await.unwrap();
        url.load(&db).await.unwrap();

        assert_eq!(restarted.path(), std::path::Path::new("/tmp/nama"));
        assert_eq!(url.get(), "https://nama.example.com");
    }

    #[tokio::test]
    async fn a_saved_remote_playback_limit_is_loaded_again_after_a_restart_and_starts_original() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let fresh = RemotePlaybackLimit::default();
        fresh.load(&db).await.unwrap();
        let out_of_the_box = fresh.get();
        fresh.save(&db, Some(PlaybackLimit::P720)).await.unwrap();

        let restarted = RemotePlaybackLimit::default();
        restarted.load(&db).await.unwrap();

        assert_eq!(out_of_the_box, None);
        assert_eq!(restarted.get(), Some(PlaybackLimit::P720));
    }

    #[tokio::test]
    async fn segment_files_go_to_the_default_until_a_folder_is_set() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let folder = SegmentFolder::new("/data/transcodes".into());
        folder.load(&db).await.unwrap();

        assert_eq!(folder.get(), "");
        assert_eq!(folder.path(), std::path::Path::new("/data/transcodes"));
    }

    #[tokio::test]
    async fn the_transcode_cap_is_unset_until_one_is_saved_which_is_loaded_again_after_a_restart() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let unsaved = MaxTranscodes::default();
        unsaved.load(&db).await.unwrap();
        assert_eq!(unsaved.get(), None);
        unsaved.save(&db, Some(5)).await.unwrap();

        let restarted = MaxTranscodes::default();
        restarted.load(&db).await.unwrap();
        assert_eq!(restarted.get(), Some(5));
        restarted.save(&db, None).await.unwrap();
        restarted.load(&db).await.unwrap();
        assert_eq!(restarted.get(), None);
    }

    #[tokio::test]
    async fn hardware_acceleration_is_automatic_until_a_choice_is_saved_and_loaded_again() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let unsaved = HardwareAcceleration::default();
        unsaved.load(&db).await.unwrap();
        assert_eq!(unsaved.get(), HardwareChoice::Automatic);

        for choice in [HardwareChoice::Api(Accel::Vaapi), HardwareChoice::Off] {
            unsaved.save(&db, choice).await.unwrap();
            let restarted = HardwareAcceleration::default();
            restarted.load(&db).await.unwrap();
            assert_eq!(restarted.get(), choice);
        }
    }

    #[test]
    fn a_request_link_keeps_the_urls_path_and_has_one_slash() {
        let url = ServerUrl::default();
        let unset = url.request_link(7);
        url.set("https://example.com/nama/".to_owned());

        assert_eq!(unset, None);
        assert_eq!(
            url.request_link(7).as_deref(),
            Some("https://example.com/nama/requests/7")
        );
    }
}
