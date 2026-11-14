//! An Installed Plugin as a title source: the Kernel's `TitleSource` calls as the contract's
//! `TitleSourceService` rpcs, and their answers back as the Kernel's.

use buffa::{Message, MessageField};
use nama_plugin_api::{
    BoxFuture,
    metadata::{
        Candidate, Episode, ExternalId, ExternalIds, Image, MetadataError, Movie, SearchQuery,
        Season, Show, TitleKind, TitleSource,
    },
};
use nama_plugin_proto::nama::plugin::v1 as wire;

use super::{
    Running,
    services::{
        Method, TITLE_EPISODE, TITLE_IMAGE, TITLE_MOVIE, TITLE_SEARCH, TITLE_SEASON, TITLE_SHOW,
    },
};

impl Running {
    /// Calls `method` if the Plugin serves it, its failure as [`MetadataError::Failed`]. One it
    /// doesn't serve answers [`MetadataError::Unsupported`] without a call.
    async fn ask<Req: Message, Resp: Message>(
        &self,
        method: &Method,
        request: &Req,
    ) -> Result<Resp, MetadataError> {
        if !self.serves(method) {
            return Err(MetadataError::Unsupported);
        }
        self.call_method(method, request)
            .await
            .map_err(MetadataError::Failed)
    }
}

impl TitleSource for Running {
    fn namespace(&self) -> &str {
        &self.package.title_namespace
    }

    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        let query = search_query(query);
        Box::pin(async move {
            let results: wire::SearchResults = self.ask(&TITLE_SEARCH, &query).await?;
            results.candidates.into_iter().map(candidate).collect()
        })
    }

    fn movie<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        let asked = wire::TitleAsked {
            id: MessageField::some(title_id(id)),
            ..Default::default()
        };
        Box::pin(async move {
            // An answer left unset means the source has no such title.
            let answer: wire::MovieAnswer = self.ask(&TITLE_MOVIE, &asked).await?;
            movie(answer.movie.ok_or(MetadataError::NotFound)?)
        })
    }

    fn show<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        let asked = wire::TitleAsked {
            id: MessageField::some(title_id(id)),
            ..Default::default()
        };
        Box::pin(async move {
            let answer: wire::ShowAnswer = self.ask(&TITLE_SHOW, &asked).await?;
            show(answer.show.ok_or(MetadataError::NotFound)?)
        })
    }

    fn season<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        let asked = wire::SeasonAsked {
            show: MessageField::some(title_id(show)),
            season,
            ..Default::default()
        };
        Box::pin(async move {
            let answer: wire::SeasonAnswer = self.ask(&TITLE_SEASON, &asked).await?;
            self::season(answer.season.ok_or(MetadataError::NotFound)?)
        })
    }

    fn episode<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
        episode: u32,
    ) -> BoxFuture<'a, Result<Episode, MetadataError>> {
        let asked = wire::EpisodeAsked {
            show: MessageField::some(title_id(show)),
            season,
            episode,
            ..Default::default()
        };
        Box::pin(async move {
            let answer: wire::EpisodeAnswer = self.ask(&TITLE_EPISODE, &asked).await?;
            self::episode(answer.episode.ok_or(MetadataError::NotFound)?)
        })
    }

    fn image<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>> {
        let asked = wire::ImageAsked {
            path: path.to_owned(),
            ..Default::default()
        };
        Box::pin(async move {
            let answer: wire::ImageAnswer = self.ask(&TITLE_IMAGE, &asked).await?;
            let image = answer.image.ok_or(MetadataError::NotFound)?;
            Ok(Image {
                content_type: image.content_type,
                bytes: image.bytes,
            })
        })
    }
}

pub(super) fn search_query(query: &SearchQuery) -> wire::SearchQuery {
    let kind = match query.kind {
        TitleKind::Movie => wire::TitleKind::TITLE_KIND_MOVIE,
        TitleKind::Show => wire::TitleKind::TITLE_KIND_SHOW,
    };
    wire::SearchQuery {
        kind: kind.into(),
        title: query.title.clone(),
        year: query.year,
        ..Default::default()
    }
}

pub(super) fn title_id(id: &ExternalId) -> wire::TitleId {
    wire::TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

/// `id` as the Kernel's, or why the source's answer is no good: a Plugin's id is checked here,
/// at the trust boundary, since the Kernel stores and links by it.
pub(super) fn external_id(id: &wire::TitleId) -> Result<ExternalId, MetadataError> {
    ExternalId::new(&id.namespace, id.value.clone()).map_err(|err| {
        let named = format!("{}-{}", id.namespace, id.value);
        MetadataError::Failed(
            format!("the title source answered with the id {named:?}: {err}").into(),
        )
    })
}

fn external_ids(ids: &[wire::TitleId]) -> Result<ExternalIds, MetadataError> {
    ids.iter().map(external_id).collect()
}

pub(super) fn candidate(candidate: wire::Candidate) -> Result<Candidate, MetadataError> {
    let id = candidate.id.as_option().ok_or_else(|| {
        MetadataError::Failed(
            format!("the title source found {:?} without an id", candidate.title).into(),
        )
    })?;
    Ok(Candidate {
        id: external_id(id)?,
        ids: external_ids(&candidate.ids)?,
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.year,
        poster: candidate.poster,
        origin_country: candidate.origin_country,
        vote_count: candidate.vote_count,
    })
}

pub(super) fn movie(movie: wire::Movie) -> Result<Movie, MetadataError> {
    Ok(Movie {
        ids: external_ids(&movie.ids)?,
        title: movie.title,
        original_title: movie.original_title,
        overview: movie.overview,
        release_date: movie.release_date,
        runtime_minutes: movie.runtime_minutes,
        poster: movie.poster,
        backdrop: movie.backdrop,
    })
}

pub(super) fn show(show: wire::Show) -> Result<Show, MetadataError> {
    Ok(Show {
        ids: external_ids(&show.ids)?,
        title: show.title,
        original_title: show.original_title,
        overview: show.overview,
        first_air_date: show.first_air_date,
        poster: show.poster,
        backdrop: show.backdrop,
        seasons: show.seasons,
    })
}

pub(super) fn season(season: wire::Season) -> Result<Season, MetadataError> {
    Ok(Season {
        ids: external_ids(&season.ids)?,
        number: season.number,
        title: season.title,
        overview: season.overview,
        air_date: season.air_date,
        poster: season.poster,
        episodes: season
            .episodes
            .into_iter()
            .map(episode)
            .collect::<Result<_, _>>()?,
    })
}

pub(super) fn episode(episode: wire::Episode) -> Result<Episode, MetadataError> {
    Ok(Episode {
        ids: external_ids(&episode.ids)?,
        season: episode.season,
        number: episode.number,
        title: episode.title,
        overview: episode.overview,
        air_date: episode.air_date,
        runtime_minutes: episode.runtime_minutes,
        still: episode.still,
    })
}

#[cfg(test)]
#[path = "title_source_tests.rs"]
mod tests;
