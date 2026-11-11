//! Asking the metadata Provider for titles and images, and what its failures mean: no Provider
//! enabled, or one that failed, is [`Error::Unavailable`]; a title or image it doesn't have, or an
//! id it can't look titles up by, is [`Error::NotFound`]. It alone asks (see the crate's
//! `AGENTS.md`).

use std::{future::Future, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{
        Candidate, ExternalId, ExternalIds, Image, MetadataError, MetadataProvider, Movie,
        SearchQuery, Season, Show,
    },
};

use crate::{contents::Metadata, plugins::Registry};

/// Why the metadata Provider gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    /// It has no such title, Season or image, or can't look titles up by the id asked for.
    #[error("the metadata source has no such title")]
    NotFound,
    /// No metadata Provider is enabled, or it failed. Worth asking again later.
    #[error("no metadata Provider could answer")]
    Unavailable,
}

/// The metadata Providers enabled in a [`Registry`], asked through the one that comes first.
#[derive(Debug, Clone)]
pub(crate) struct MetadataSource {
    plugins: Registry,
}

impl MetadataSource {
    pub(crate) const fn new(plugins: Registry) -> Self {
        Self { plugins }
    }

    /// The titles the Provider finds for `query`, best fit first, with the Plugin whose Provider
    /// found them, which their image paths are for.
    pub(crate) async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<(PluginId, Vec<Candidate>), Error> {
        self.ask(|metadata| {
            let query = query.clone();
            async move { metadata.search(&query).await }
        })
        .await
    }

    /// The metadata of the Movie with the id `id`.
    pub(crate) async fn movie(&self, id: &ExternalId) -> Result<Metadata, Error> {
        let (plugin, movie) = self
            .ask(|metadata| {
                let id = id.clone();
                async move { metadata.movie(&id).await }
            })
            .await?;
        Ok(Metadata::movie(plugin, movie))
    }

    /// The Show with the id `id`, and the Plugin whose Provider gave it.
    pub(crate) async fn show(&self, id: &ExternalId) -> Result<(PluginId, Show), Error> {
        self.ask(|metadata| {
            let id = id.clone();
            async move { metadata.show(&id).await }
        })
        .await
    }

    /// The Season `number` of the Show with the id `show`, with its Episodes.
    pub(crate) async fn season(&self, show: &ExternalId, number: u32) -> Result<Season, Error> {
        self.ask(|metadata| {
            let show = show.clone();
            async move { metadata.season(&show, number).await }
        })
        .await
        .map(|(_, season)| season)
    }

    /// The image at `path` from the Plugin `plugin`'s Provider, which handed the path out.
    pub(crate) async fn image(&self, plugin: &PluginId, path: &str) -> Result<Image, Error> {
        let answered = self
            .plugins
            .call_provider_of::<dyn MetadataProvider, _, _>(plugin, |metadata| {
                let path = path.to_owned();
                async move { metadata.image(&path).await }
            })
            .await;
        read(answered.map(|answer| (plugin.clone(), answer))).map(|(_, image)| image)
    }

    async fn ask<R, F>(
        &self,
        call: impl Fn(Arc<dyn MetadataProvider>) -> F,
    ) -> Result<(PluginId, R), Error>
    where
        F: Future<Output = Result<R, MetadataError>>,
    {
        read(
            self.plugins
                .call_provider_with_id::<dyn MetadataProvider, _, _>(call)
                .await,
        )
    }
}

/// What the Provider of the Plugin that answered said, or `None` if no Provider could be asked.
fn read<R>(answered: Option<(PluginId, Result<R, MetadataError>)>) -> Result<(PluginId, R), Error> {
    match answered {
        Some((plugin, Ok(answer))) => Ok((plugin, answer)),
        Some((_, Err(MetadataError::NotFound | MetadataError::UnsupportedId(_)))) => {
            Err(Error::NotFound)
        }
        Some((plugin, Err(MetadataError::Failed(error)))) => {
            tracing::warn!(%plugin, %error, "the metadata provider failed");
            Err(Error::Unavailable)
        }
        None => {
            tracing::debug!("no metadata provider could be asked");
            Err(Error::Unavailable)
        }
    }
}

impl Metadata {
    fn movie(plugin: PluginId, movie: Movie) -> Self {
        Self {
            plugin,
            ids: movie.ids,
            show_ids: ExternalIds::default(),
            title: movie.title,
            original_title: movie.original_title,
            show_title: None,
            overview: movie.overview,
            release_date: movie.release_date,
            runtime_minutes: movie.runtime_minutes,
            poster: movie.poster,
            backdrop: movie.backdrop,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nama_plugin_api::{
        BoxFuture, Manifest, Plugin, PluginId, PointKey, Priority, Registrar,
        metadata::{
            Candidate, Episode, ExternalId, Image, MetadataError, MetadataProvider, Movie,
            SearchQuery, Season, Show,
        },
        settings::SettingsSchema,
    };

    use super::{Error, MetadataSource};
    use crate::plugins::Registry;

    /// A metadata Provider that answers every question with the error `.0` makes.
    struct Failing(fn() -> MetadataError);

    impl MetadataProvider for Failing {
        fn search<'a>(
            &'a self,
            _query: &'a SearchQuery,
        ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }

        fn movie<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }

        fn show<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }

        fn season<'a>(
            &'a self,
            _show: &'a ExternalId,
            _season: u32,
        ) -> BoxFuture<'a, Result<Season, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }

        fn episode<'a>(
            &'a self,
            _show: &'a ExternalId,
            _season: u32,
            _episode: u32,
        ) -> BoxFuture<'a, Result<Episode, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }

        fn image<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>> {
            Box::pin(async { Err(self.0()) })
        }
    }

    struct FailingPlugin(fn() -> MetadataError);

    impl Plugin for FailingPlugin {
        fn manifest(&self) -> Manifest {
            Manifest {
                id: PluginId::from_static("failing"),
                name: String::new(),
                description: String::new(),
                version: "0.0.0".to_owned(),
                settings: SettingsSchema::default(),
                extension_points: vec![PointKey::provide::<dyn MetadataProvider>()],
                logo_svg: None,
            }
        }

        fn register(&self, registrar: &mut Registrar) {
            registrar.provide::<dyn MetadataProvider>(Priority::DEFAULT, Arc::new(Failing(self.0)));
        }
    }

    /// What asking for a Show, then for an image from the Plugin `failing`, fails with when its
    /// Provider fails with `failure`, or when there is no Provider.
    async fn asking(failure: Option<fn() -> MetadataError>) -> (Error, Error) {
        let plugins = Registry::new();
        if let Some(failure) = failure {
            plugins.enable(Arc::new(FailingPlugin(failure))).unwrap();
        }
        let source = MetadataSource::new(plugins);
        let show = source.show(&ExternalId::Tmdb(1399)).await.unwrap_err();
        let image = source
            .image(&PluginId::from_static("failing"), "/poster.jpg")
            .await
            .unwrap_err();
        (show, image)
    }

    #[tokio::test]
    async fn without_a_provider_the_source_is_unavailable() {
        assert_eq!(asking(None).await, (Error::Unavailable, Error::Unavailable));
    }

    #[tokio::test]
    async fn a_title_the_provider_does_not_have_is_not_found() {
        let failure = || MetadataError::NotFound;

        assert_eq!(
            asking(Some(failure)).await,
            (Error::NotFound, Error::NotFound)
        );
    }

    #[tokio::test]
    async fn an_id_the_provider_cannot_look_up_is_not_found() {
        let failure = || MetadataError::UnsupportedId(ExternalId::Imdb("tt0944947".to_owned()));

        assert_eq!(
            asking(Some(failure)).await,
            (Error::NotFound, Error::NotFound)
        );
    }

    #[tokio::test]
    async fn a_provider_that_fails_is_unavailable() {
        let failure = || MetadataError::Failed("TMDB is unreachable".into());

        assert_eq!(
            asking(Some(failure)).await,
            (Error::Unavailable, Error::Unavailable)
        );
    }
}
