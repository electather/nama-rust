//! What a Match stores of a title beyond its title source's answer: the artwork picked for it,
//! for an Episode what it takes of its Show, what the Similar sources answer about it, and the
//! Movies its Box Set lists.

use std::collections::HashMap;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::{Matcher, Outcomes};
use crate::library::{
    DEFAULT_ORDER, ImageRef, ListedMovie, Metadata, Picked, SimilarAnswer, SourceEpisode,
    SourceSeason, SourceShow, Unanswered, primary_id,
};

impl Matcher {
    /// `outcomes` with what each Match stores beyond its title source's answer, looked up before
    /// the write that stores them. Every path that stores a Match takes them here, so none skips
    /// one.
    pub(super) async fn with_lookups(&self, outcomes: Outcomes) -> Outcomes {
        let outcomes = self.with_similar(outcomes).await;
        self.with_box_set_movies(outcomes).await
    }

    /// `outcomes` with what each enabled Similar source answers about each Matched title, a Movie
    /// or an Episode's Show, asked once per title, before the write that stores them (ADR 0084).
    async fn with_similar(&self, mut outcomes: Outcomes) -> Outcomes {
        let mut answered: HashMap<ExternalIds, Vec<SimilarAnswer>> = HashMap::new();
        for (_, outcome) in &mut outcomes.items {
            let Ok(matched) = outcome else { continue };
            let (kind, ids) = matched.title();
            let ids = ids.clone();
            if !answered.contains_key(&ids) {
                let (genres, roles) = (&matched.genres, &matched.roles);
                let answers = self.similar.ask(kind, &ids, genres, roles).await;
                answered.insert(ids.clone(), answers);
            }
            matched.similar = answered.get(&ids).cloned().unwrap_or_default();
        }
        outcomes
    }

    /// `outcomes` with the Movies each Matched Movie's Box Set lists, as the title source that
    /// Matched it has them, looked up once per Box Set before the write that stores them, as a
    /// Home row's missing titles are. Every one is looked up, those on the Server too, since
    /// whether the Server has one is decided as a User looks, and it may lose one.
    async fn with_box_set_movies(&self, mut outcomes: Outcomes) -> Outcomes {
        let mut looked_up: HashMap<ExternalId, Option<Vec<ListedMovie>>> = HashMap::new();
        for (_, outcome) in &mut outcomes.items {
            let Ok(matched) = outcome else { continue };
            let Some(id) = matched.box_set.as_ref().map(|box_set| box_set.id.clone()) else {
                continue;
            };
            if !looked_up.contains_key(&id) {
                let listed = self.listed_movies(matched).await;
                looked_up.insert(id.clone(), listed);
            }
            if let Some(box_set) = &mut matched.box_set {
                box_set.looked_up = looked_up.get(&id).cloned().flatten();
            }
        }
        outcomes
    }

    /// The Movies `matched`'s Box Set lists, as the title source that Matched it has them now,
    /// leaving out those it doesn't have, or `None` once it fails. `matched` itself is taken as
    /// it was just fetched.
    async fn listed_movies(&self, matched: &Metadata) -> Option<Vec<ListedMovie>> {
        let listed = matched
            .box_set
            .as_ref()
            .map_or(&[][..], |box_set| &box_set.movies);
        let source = self.only(matched.plugin.clone());
        let mut found = Vec::new();
        // ponytail: one title-source call per listed Movie, one after another, at every Match
        // and refresh, as a Home row's missing titles are.
        for id in listed {
            if matched.ids.iter().any(|known| known == *id) {
                found.push(ListedMovie::of(id, matched));
                continue;
            }
            match source.metadata.movie(id).await {
                Ok(movie) => found.push(ListedMovie::of(id, &movie)),
                Err(Unanswered::NotFound) => {}
                Err(Unanswered::Unavailable) => return None,
            }
        }
        Some(found)
    }
}

impl ListedMovie {
    /// The Movie the Box Set lists as `listed_as`, as its title source gave it.
    fn of(listed_as: &ExternalId, movie: &Metadata) -> Self {
        let id = primary_id(&movie.ids, &DEFAULT_ORDER).unwrap_or_else(|| listed_as.clone());
        Self {
            listed_as: listed_as.to_string(),
            id: id.to_string(),
            year: (movie.release_date.as_deref()).and_then(|date| date.get(..4)?.parse().ok()),
            title: movie.title.clone(),
            poster: movie.poster.as_ref().map(|poster| poster.path.clone()),
        }
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
            genre_ids: show.genre_ids.clone(),
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
            box_set: None,
        }
    }
}
