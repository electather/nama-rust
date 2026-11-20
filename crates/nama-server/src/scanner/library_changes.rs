//! Creating, updating and deleting a Library: refusing an order of title sources (ADR 0044) or
//! Recognisers (ADR 0050) it may not ask its Plugins in, and what must follow each change.

use nama_plugin_api::{PluginId, PointKey, recogniser::Recogniser};

use super::Scanner;
use crate::{
    db::detached,
    library::{Library, LibraryError, LibraryKind, Order, check_known, default_title_sources},
};

impl Scanner {
    /// Creates a Library asking its Plugins in `order`, and watches its folders. An order naming
    /// no title source gets the default. The watch follows even if the caller stops waiting
    /// (ADR 0029); scanning the new Library is left to the caller.
    ///
    /// Refuses an order as [`Self::check_order`] does.
    pub(crate) async fn create_library(
        &self,
        name: String,
        kind: LibraryKind,
        folders: Vec<String>,
        mut order: Order,
    ) -> Result<Library, LibraryError> {
        if order.title_sources.is_empty() {
            order.title_sources = default_title_sources();
        }
        let scanner = self.clone();
        detached(async move {
            scanner.check_order(&order, &[]).await?;
            let libraries = &scanner.inner.libraries;
            let library = (libraries.create_in_order(&name, kind, folders, order)).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Renames the Library `id`, replaces its folders and whichever of its orders `order` gives,
    /// then watches its new folders and not its old ones, even if the caller stops waiting (ADR
    /// 0029). A new order doesn't Match its titles again.
    ///
    /// Refuses an order as [`Self::check_order`] does, judging only what changes of the stored
    /// title sources.
    pub(crate) async fn update_library(
        &self,
        id: i64,
        name: String,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, LibraryError> {
        let scanner = self.clone();
        detached(async move {
            let libraries = &scanner.inner.libraries;
            let stored = if order.title_sources.is_empty() {
                Vec::new()
            } else {
                libraries.get(id).await?.title_sources
            };
            scanner.check_order(&order, &stored).await?;
            let library = libraries.update(id, &name, folders, order).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Deletes the Library `id`, then throws away its Versions' cached subtitles where something
    /// probes, the images only its Matches stored, its Custom Image and its copies of the images
    /// beside its media, and stops watching its folders, even if the caller stops waiting
    /// (ADR 0029).
    pub(crate) async fn delete_library(&self, id: i64) -> Result<(), LibraryError> {
        let scanner = self.clone();
        detached(async move {
            scanner.inner.libraries.delete(id).await?;
            scanner
                .inner
                .artwork
                .drop_unused(&scanner.inner.contents)
                .await;
            scanner.inner.custom_images.delete_library(id).await;
            scanner.inner.local_images.delete_library(id).await;
            if let Some(probing) = &scanner.inner.probing {
                probing.clean_subtitles().await;
            }
            scanner.rewatch().await;
            Ok(())
        })
        .await
    }

    /// Refuses `order`, given in place of a Library's `stored` title sources, if its title sources
    /// fail [`check_known`] against every Plugin's, or its Recognisers name an id twice, one with
    /// a comma, or one of no Plugin, enabled or not, whose Manifest names the Recogniser point.
    /// What `order` leaves out keeps the stored order, so isn't judged.
    async fn check_order(&self, order: &Order, stored: &[String]) -> Result<(), LibraryError> {
        if !order.title_sources.is_empty() {
            let known = self.inner.plugin_settings.title_sources();
            check_known(&order.title_sources, stored, &known)?;
        }
        let ids = order.recognisers.as_deref().unwrap_or_default();
        for (seen, id) in (1..).zip(ids) {
            let repeated = ids.iter().take(seen).filter(|other| *other == id).count() > 1;
            // A stored order is comma-separated.
            if repeated || id.contains(',') || !self.is_recogniser(id).await? {
                return Err(LibraryError::RecognisersInvalid { plugin: id.clone() });
            }
        }
        Ok(())
    }

    async fn is_recogniser(&self, id: &str) -> Result<bool, LibraryError> {
        let plugin = PluginId::from(id.to_owned());
        let stored = self.inner.plugin_settings.get(&plugin).await?;
        let point = PointKey::provide::<dyn Recogniser>();
        Ok(stored.is_some_and(|stored| {
            (stored.plugin.manifest)
                .is_ok_and(|manifest| manifest.extension_points.contains(&point))
        }))
    }
}

#[cfg(test)]
#[path = "library_changes_tests.rs"]
mod tests;
