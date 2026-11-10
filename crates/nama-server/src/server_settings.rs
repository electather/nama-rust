//! What an admin sets about the Server as a whole, kept in the database: so far its URL, which
//! notifications link to the web app with, and the folder playback writes segment files to.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::settings::{SettingKind, SettingValue};

use crate::db::Database;

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

#[cfg(test)]
mod tests {
    use super::{SegmentFolder, ServerUrl};
    use crate::db::Database;

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
    async fn segment_files_go_to_the_default_until_a_folder_is_set() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let folder = SegmentFolder::new("/data/transcodes".into());
        folder.load(&db).await.unwrap();

        assert_eq!(folder.get(), "");
        assert_eq!(folder.path(), std::path::Path::new("/data/transcodes"));
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
