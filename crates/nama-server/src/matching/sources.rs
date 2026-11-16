//! Asking a Library's title sources, in its order, for the title an item names (ADR 0042's open
//! ids): a pin or an ID Hint is fetched from the source keeping its namespace, then those looking
//! titles up by it, and a title and year are searched for with the first source that answers.
//! Before acting on what the search decided, Matching asks the Recognisers, and a title one names
//! is fetched as an ID Hint's is.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    FoundShow, Miss, TitleKey,
    confidence::{self, Decision},
    recognisers::{Recognisers, Searched},
};
use crate::library::{
    Candidate, Metadata, Name, Pin, SearchQuery, Source, SourceSeason, TiedTitle, TitleKind,
    TitleSources, Unanswered, UnmatchedReason, country_tag,
};

/// The title an item names, before its details are fetched.
#[derive(Debug)]
struct Found {
    /// The id to fetch it by.
    id: ExternalId,
    /// The ids the source that found it knows it by.
    ids: ExternalIds,
    /// Whether `id` is the item's ID Hint, which a miss is then blamed on.
    from_hint: bool,
    /// The sources `id` routes to, when finding it already asked; else routed when fetched.
    routed: Option<Vec<Source>>,
}

impl Found {
    fn id(id: ExternalId, from_hint: bool, routed: Vec<Source>) -> Self {
        Self {
            id,
            ids: ExternalIds::default(),
            from_hint,
            routed: Some(routed),
        }
    }

    /// The sources to fetch this title of `kind` from.
    async fn route(&mut self, metadata: &TitleSources, kind: TitleKind) -> Vec<Source> {
        match self.routed.take() {
            Some(routed) => routed,
            None => {
                metadata
                    .by_namespace(self.id.namespace(), kind.method())
                    .await
            }
        }
    }

    /// `err`, from fetching this title, as a miss: blamed on the ID Hint if that named it.
    fn missed(&self, err: Unanswered) -> Miss {
        match Miss::from(err) {
            Miss::Unmatched(None) if self.from_hint => {
                Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
            }
            miss @ (Miss::Unmatched(_)
            | Miss::Undecided
            | Miss::RecogniserFailed
            | Miss::Unserved) => miss,
        }
    }
}

/// The Movie `key` names, from `metadata`'s sources in their order, its files at `paths`.
pub(super) async fn movie(
    metadata: &TitleSources,
    recognisers: &Recognisers,
    key: &TitleKey,
    paths: &[String],
) -> Result<Metadata, Miss> {
    let mut found = find(metadata, recognisers, TitleKind::Movie, key, paths).await?;
    let routed = found.route(metadata, TitleKind::Movie).await;
    movie_from(metadata, routed, &found.id, found.ids.clone())
        .await
        .map_err(|err| found.missed(err))
}

/// The Show `key` names, from `metadata`'s sources in their order, its Episodes' files at `paths`.
pub(super) async fn show(
    metadata: &TitleSources,
    recognisers: &Recognisers,
    key: &TitleKey,
    paths: &[String],
) -> Result<FoundShow, Miss> {
    let mut found = find(metadata, recognisers, TitleKind::Show, key, paths).await?;
    let routed = found.route(metadata, TitleKind::Show).await;
    show_from(metadata, routed, &found.id, found.ids.clone())
        .await
        .map_err(|err| found.missed(err))
}

/// The Movie with the id `id`, from the source keeping its namespace or else the first looking it
/// up to have it, with every id `known` for it.
pub(super) async fn fetch_movie(
    metadata: &TitleSources,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<Metadata, Unanswered> {
    let routed = metadata
        .by_namespace(id.namespace(), TitleKind::Movie.method())
        .await;
    movie_from(metadata, routed, id, known).await
}

/// [`fetch_movie`] from the sources `routed` to.
async fn movie_from(
    metadata: &TitleSources,
    routed: Vec<Source>,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<Metadata, Unanswered> {
    let (_, mut movie) = metadata.movie_from(routed, id).await?;
    movie.ids = merged(id, known, &movie.ids);
    Ok(movie)
}

/// The Show with the id `id`, routed as [`fetch_movie`] routes a Movie, and the id its Seasons are
/// fetched by from the source that gave it: its id in that source's namespace, else `id`.
pub(super) async fn fetch_show(
    metadata: &TitleSources,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<FoundShow, Unanswered> {
    let routed = metadata
        .by_namespace(id.namespace(), TitleKind::Show.method())
        .await;
    show_from(metadata, routed, id, known).await
}

/// [`fetch_show`] from the sources `routed` to.
async fn show_from(
    metadata: &TitleSources,
    routed: Vec<Source>,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<FoundShow, Unanswered> {
    let (source, mut show) = metadata.show_from(routed, id).await?;
    show.ids = merged(id, known, &show.ids);
    let id = show
        .ids
        .get(&source.namespace)
        .unwrap_or_else(|| id.clone());
    Ok(FoundShow { source, id, show })
}

/// The Season `number` of the Show `show`, from the source that gave the Show.
pub(super) async fn season(
    metadata: &TitleSources,
    show: &FoundShow,
    number: i64,
) -> Result<SourceSeason, Miss> {
    let number = u32::try_from(number).map_err(|_negative| Miss::Unmatched(None))?;
    Ok(metadata.season_from(&show.source, &show.id, number).await?)
}

/// Every id a title is known by: `known`, the id `asked` that a source answered for, and the
/// answering source's `answered`, which win where two give the same namespace.
fn merged(asked: &ExternalId, known: ExternalIds, answered: &ExternalIds) -> ExternalIds {
    let mut ids = known;
    ids.insert(asked.clone());
    for id in answered.iter() {
        ids.insert(id);
    }
    ids
}

/// The title of `kind` `key` names: what an admin pinned, its ID Hint where a source looks titles
/// up by its namespace, or else what searching for it and asking the Recognisers find.
async fn find(
    metadata: &TitleSources,
    recognisers: &Recognisers,
    kind: TitleKind,
    key: &TitleKey,
    paths: &[String],
) -> Result<Found, Miss> {
    match &key.pin {
        Some(Pin::Title(id)) => {
            let routed = metadata.by_namespace(id.namespace(), kind.method()).await;
            // A pin waits for a source looking up its namespace rather than Matching another title.
            if routed.is_empty() {
                return Err(Miss::Unserved);
            }
            return Ok(Found::id(id.clone(), false, routed));
        }
        Some(Pin::Unmatched) => return Err(Miss::Unmatched(None)),
        None => {}
    }
    if let Some(hint) = &key.name.hint {
        // A curly tag such as `{x264-GROUP}` parses as an ID Hint no source looks up by, so the
        // item is searched for as if it had none.
        let routed = metadata.by_namespace(hint.namespace(), kind.method()).await;
        if !routed.is_empty() {
            return Ok(Found::id(hint.clone(), true, routed));
        }
    }
    search(metadata, recognisers, kind, &key.name, paths).await
}

/// The title the first Recogniser to name one a source serves names, else the search result that
/// fits `name` confidently and unambiguously, from the first source to find any.
async fn search(
    metadata: &TitleSources,
    recognisers: &Recognisers,
    kind: TitleKind,
    name: &Name,
    paths: &[String],
) -> Result<Found, Miss> {
    let year = name.year.and_then(|year| i32::try_from(year).ok());
    let (title, country) = match (kind, country_tag(&name.title)) {
        (TitleKind::Show, Some((title, country))) => (title, Some(country)),
        _ => (name.title.as_str(), None),
    };
    let mut query = SearchQuery {
        kind,
        title: title.to_owned(),
        year,
    };
    let mut candidates = searched(metadata, &query, country).await?;
    let mut decision = confidence::decide(title, year, &candidates);
    // A metadata source may only return titles from exactly the year searched for, so a year
    // off by one (release dates differ by country) finds nothing. Search again without it,
    // and let the confidence score weigh the year.
    if decision == Decision::NoMatch && year.is_some() {
        query.year = None;
        candidates = searched(metadata, &query, country).await?;
        decision = confidence::decide(title, year, &candidates);
    }
    let searched = Searched {
        kind,
        title,
        year,
        paths,
        candidates: &candidates,
    };
    if let Some(recognised) = recognisers.recognise(metadata, &searched).await? {
        let mut found = Found::id(recognised.id, false, recognised.routed);
        found.ids = recognised.ids;
        return Ok(found);
    }
    match decision {
        Decision::Match(id) => {
            let ids = (candidates.into_iter())
                .find(|candidate| candidate.id == id)
                .map(|candidate| candidate.ids)
                .unwrap_or_default();
            Ok(Found {
                id,
                ids,
                from_hint: false,
                routed: None,
            })
        }
        Decision::Ambiguous(tied) => {
            tracing::info!(
                title = %name.title,
                year = ?name.year,
                "more than one title fits; left Unmatched"
            );
            let tied = tied.into_iter().map(TiedTitle::from).collect();
            Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(tied))))
        }
        Decision::NoMatch => Err(Miss::Unmatched(Some(UnmatchedReason::NoTitleFits))),
    }
}

/// What the first source to find any finds for `query`, keeping only titles from `country` if
/// given; nothing when no source finds any.
async fn searched(
    metadata: &TitleSources,
    query: &SearchQuery,
    country: Option<&str>,
) -> Result<Vec<Candidate>, Miss> {
    let mut candidates = match metadata.search_in_turn(query).await {
        Ok((_, candidates)) => candidates,
        Err(Unanswered::NotFound) => Vec::new(),
        Err(Unanswered::Unavailable) => return Err(Miss::Undecided),
    };
    if let Some(country) = country {
        candidates.retain(|candidate| candidate.origin_country.iter().any(|c| c == country));
    }
    Ok(candidates)
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
