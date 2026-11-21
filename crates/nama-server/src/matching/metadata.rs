//! What a Match stores of a title beyond its title source's answer: the artwork picked for it,
//! for an Episode what it takes of its Show, and what the Similar sources answer about it.

use std::collections::HashMap;

use nama_plugin_api::{PluginId, metadata::ExternalIds};

use super::{Matcher, Outcomes};
use crate::library::{
    ImageRef, Metadata, Picked, SimilarAnswer, SourceEpisode, SourceSeason, SourceShow,
};

impl Matcher {
    /// `outcomes` with what each enabled Similar source answers about each Matched title, a Movie
    /// or an Episode's Show, asked once per title, before the write that stores them (ADR 0084).
    pub(super) async fn with_similar(&self, mut outcomes: Outcomes) -> Outcomes {
        let mut answered: HashMap<ExternalIds, Vec<SimilarAnswer>> = HashMap::new();
        for (_, outcome) in &mut outcomes.items {
            let Ok(matched) = outcome else { continue };
            let (kind, ids) = matched.title();
            let ids = ids.clone();
            if !answered.contains_key(&ids) {
                let answers = self.similar.ask(kind, &ids).await;
                answered.insert(ids.clone(), answers);
            }
            matched.similar = answered.get(&ids).cloned().unwrap_or_default();
        }
        outcomes
    }
}

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
            similar: Vec::new(),
            trailers: show.trailers.clone(),
            roles: show.roles.clone(),
        }
    }
}
