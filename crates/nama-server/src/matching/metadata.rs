//! What a Match stores of a title beyond its title source's answer: the artwork picked for it,
//! and for an Episode, what it takes of its Show.

use nama_plugin_api::PluginId;

use crate::library::{ImageRef, Metadata, Picked, SourceEpisode, SourceSeason, SourceShow};

impl Metadata {
    /// This title with the artwork `picked`, keeping its own poster or backdrop where none was.
    pub(super) fn with_picked(self, picked: &Picked) -> Self {
        let picked = picked.clone();
        Self {
            poster: picked.poster.or(self.poster),
            backdrop: picked.backdrop.or(self.backdrop),
            titled_backdrop: picked.titled_backdrop,
            logo: picked.logo.or(self.logo),
            ..self
        }
    }

    /// An Episode of `show`, with its Season's own poster, else the Show's `picked` artwork.
    pub(super) fn episode(
        plugin: &PluginId,
        (show, picked): (&SourceShow, &Picked),
        season: &SourceSeason,
        episode: &SourceEpisode,
    ) -> Self {
        let poster = ImageRef::of(plugin, season.poster.as_ref());
        Self {
            plugin: plugin.clone(),
            ids: episode.ids.clone(),
            show_ids: show.ids.clone(),
            title: episode.title.clone(),
            original_title: None,
            show_title: Some(show.title.clone()),
            overview: episode.overview.clone(),
            release_date: episode.air_date.clone(),
            runtime_minutes: episode.runtime_minutes,
            poster: (poster.or_else(|| picked.poster.clone()))
                .or_else(|| ImageRef::of(plugin, show.poster.as_ref())),
            backdrop: (picked.backdrop.clone())
                .or_else(|| ImageRef::of(plugin, show.backdrop.as_ref())),
            titled_backdrop: picked.titled_backdrop.clone(),
            logo: picked.logo.clone(),
            still: episode.still.clone(),
            genres: show.genres.clone(),
            content_rating: show.content_rating.clone(),
            show_poster: (picked.poster.clone())
                .or_else(|| ImageRef::of(plugin, show.poster.as_ref())),
            show_overview: show.overview.clone(),
            show_first_air_date: show.first_air_date.clone(),
            score: show.score,
            original_language: show.original_language.clone(),
            similar: show.similar.clone(),
            trailers: show.trailers.clone(),
            roles: show.roles.clone(),
        }
    }
}
