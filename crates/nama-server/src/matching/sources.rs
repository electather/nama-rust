//! Asking a Library's title sources, in its order, for the title an item names (ADR 0042's open
//! ids): a pin or an ID Hint is fetched from the source keeping its namespace, then those looking
//! titles up by it, and a title and year are searched for with the first source that answers.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    FoundShow, Miss, TitleKey,
    confidence::{self, Decision},
};
use crate::{
    library::contents::{Metadata, Name, Pin, TiedTitle, UnmatchedReason},
    library::title_source::{self, Candidate, MetadataSource, SearchQuery, Season, TitleKind},
    scanner::country_tag,
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
}

impl Found {
    fn id(id: ExternalId, from_hint: bool) -> Self {
        Self {
            id,
            ids: ExternalIds::default(),
            from_hint,
        }
    }

    /// `err`, from fetching this title, as a miss: blamed on the ID Hint if that named it.
    fn missed(&self, err: title_source::Error) -> Miss {
        match Miss::from(err) {
            Miss::Unmatched(None) if self.from_hint => {
                Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
            }
            miss @ (Miss::Unmatched(_) | Miss::Undecided | Miss::Unserved) => miss,
        }
    }
}

/// The Movie `key` names, from `metadata`'s sources in their order.
pub(super) async fn movie(metadata: &MetadataSource, key: &TitleKey) -> Result<Metadata, Miss> {
    let found = find(metadata, TitleKind::Movie, key).await?;
    fetch_movie(metadata, &found.id, found.ids.clone())
        .await
        .map_err(|err| found.missed(err))
}

/// The Show `key` names, from `metadata`'s sources in their order.
pub(super) async fn show(metadata: &MetadataSource, key: &TitleKey) -> Result<FoundShow, Miss> {
    let found = find(metadata, TitleKind::Show, key).await?;
    fetch_show(metadata, &found.id, found.ids.clone())
        .await
        .map_err(|err| found.missed(err))
}

/// The Movie with the id `id`, from the source keeping its namespace or else the first looking it
/// up to have it, with every id `known` for it.
pub(super) async fn fetch_movie(
    metadata: &MetadataSource,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<Metadata, title_source::Error> {
    let (_, mut movie) = metadata.movie_by(id).await?;
    movie.ids = merged(id, known, &movie.ids);
    Ok(movie)
}

/// The Show with the id `id`, routed as [`fetch_movie`] routes a Movie, and the id its Seasons are
/// fetched by from the source that gave it: its id in that source's namespace, else `id`.
pub(super) async fn fetch_show(
    metadata: &MetadataSource,
    id: &ExternalId,
    known: ExternalIds,
) -> Result<FoundShow, title_source::Error> {
    let (source, mut show) = metadata.show_by(id).await?;
    show.ids = merged(id, known, &show.ids);
    let id = show
        .ids
        .get(&source.namespace)
        .unwrap_or_else(|| id.clone());
    Ok(FoundShow { source, id, show })
}

/// The Season `number` of the Show `show`, from the source that gave the Show.
pub(super) async fn season(
    metadata: &MetadataSource,
    show: &FoundShow,
    number: i64,
) -> Result<Season, Miss> {
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

/// The title `key` names: what an admin pinned, its ID Hint where a source looks titles up by
/// its namespace, or else the search result that fits it.
async fn find(metadata: &MetadataSource, kind: TitleKind, key: &TitleKey) -> Result<Found, Miss> {
    match &key.pin {
        Some(Pin::Title(id)) if metadata.serves(id.namespace()).await => {
            return Ok(Found::id(id.clone(), false));
        }
        // A pin waits for a source looking up its namespace rather than Matching another title.
        Some(Pin::Title(_)) => return Err(Miss::Unserved),
        Some(Pin::Unmatched) => return Err(Miss::Unmatched(None)),
        None => {}
    }
    if let Some(hint) = &key.name.hint {
        // A curly tag such as `{x264-GROUP}` parses as an ID Hint no source looks up by, so the
        // item is searched for as if it had none.
        if metadata.serves(hint.namespace()).await {
            return Ok(Found::id(hint.clone(), true));
        }
    }
    search(metadata, kind, &key.name).await
}

/// The search result that fits `name` confidently and unambiguously, from the first source to
/// find any.
async fn search(metadata: &MetadataSource, kind: TitleKind, name: &Name) -> Result<Found, Miss> {
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
    metadata: &MetadataSource,
    query: &SearchQuery,
    country: Option<&str>,
) -> Result<Vec<Candidate>, Miss> {
    let mut candidates = match metadata.search_in_turn(query).await {
        Ok((_, candidates)) => candidates,
        Err(title_source::Error::NotFound) => Vec::new(),
        Err(title_source::Error::Unavailable) => return Err(Miss::Undecided),
    };
    if let Some(country) = country {
        candidates.retain(|candidate| candidate.origin_country.iter().any(|c| c == country));
    }
    Ok(candidates)
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
