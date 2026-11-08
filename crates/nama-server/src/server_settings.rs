//! What an admin sets about the Server as a whole, kept in the database: so far its URL, which
//! notifications link to the web app with.

use std::sync::{Arc, PoisonError, RwLock};

use crate::db::Database;

/// The address people reach the Server at, or `""` for none, held in memory so that building a
/// notification doesn't read the database. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerUrl(Arc<RwLock<String>>);

impl ServerUrl {
    /// Takes the stored URL, if any, as this one.
    pub(crate) async fn load(&self, db: &Database) -> Result<(), sqlx::Error> {
        let stored = sqlx::query_scalar!("SELECT url FROM server_settings")
            .fetch_optional(db.reader())
            .await?;
        self.set(stored.unwrap_or_default());
        Ok(())
    }

    /// Stores `url`, which the caller checked is an http or https URL or empty, and uses it from
    /// now on.
    pub(crate) async fn save(&self, db: &Database, url: String) -> Result<(), sqlx::Error> {
        let stored = url.clone();
        db.write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO server_settings (id, url) VALUES (1, ?)
                     ON CONFLICT (id) DO UPDATE SET url = excluded.url",
                    stored
                )
                .execute(&mut **tx)
                .await?;
                Ok::<_, sqlx::Error>(())
            })
        })
        .await?;
        self.set(url);
        Ok(())
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
        (!url.is_empty()).then(|| format!("{}/requests#request-{id}", url.trim_end_matches('/')))
    }

    fn set(&self, url: String) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = url;
    }
}

#[cfg(test)]
mod tests {
    use super::ServerUrl;
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

    #[test]
    fn a_request_link_keeps_the_urls_path_and_has_one_slash() {
        let url = ServerUrl::default();
        let unset = url.request_link(7);
        url.set("https://example.com/nama/".to_owned());

        assert_eq!(unset, None);
        assert_eq!(
            url.request_link(7).as_deref(),
            Some("https://example.com/nama/requests#request-7")
        );
    }
}
