//! The Top Picks Installed Plugin: gives each User the "Top picks" and "Family night" Home rows,
//! and suggests their best picks for Featured (ADR 0072), ranked from what they played and their
//! Watchlist through the Kernel's functions. The Server builds it for wasm32 and bundles it in its
//! binary.

use std::collections::{HashMap, HashSet};

use nama_pdk::{
    FeaturedSuggestion, FeaturedSuggestions, HomeRow, HomeRowSource, HomeRowTitle, HomeRows,
    Manifest, Permission, SettingField, Title, TitleId, TitleKind, TitleRefKind, TitleWatchStates,
    UserFeaturedAsked, UserRowsAsked, WatchlistTitle, extism_pdk::Error, kernel, services,
};

// A new key, not the free-text `family_ratings`: the Server keeps that one's stored text, unused.
// No released Server holds it, so nothing converts it.
const FAMILY_RATINGS: &str = "family_content_ratings";
/// The Content Ratings an admin may pick for Family night, the US film and TV boards', in the
/// order a form lists them.
const RATINGS: [&str; 11] = [
    "G", "PG", "PG-13", "R", "NC-17", "TV-Y", "TV-Y7", "TV-G", "TV-PG", "TV-14", "TV-MA",
];
const DEFAULT_FAMILY_RATINGS: [&str; 6] = ["G", "PG", "PG-13", "TV-Y", "TV-G", "TV-PG"];

/// How long the rows and suggestions stay fresh, in seconds: the 15 minutes the Kernel holds a
/// shorter time to, since it asks again only once they're stale, and a title the User started
/// shows in them until then.
const FRESH_FOR_SECONDS: u32 = 15 * 60;

/// How many picks Featured is suggested.
const FEATURED_PICKS: usize = 5;
/// The top pick's weight, under the 0.8 of the Kernel's Continue Watching and over the 0.5 of
/// its new titles (ADR 0072). Each pick after it weighs [`FEATURED_STEP`] less.
const FEATURED_TOP_WEIGHT: f32 = 0.6;
const FEATURED_STEP: f32 = 0.05;

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let family_ratings = SettingField {
        default_value: nama_pdk::picked_default(DEFAULT_FAMILY_RATINGS),
        ..nama_pdk::choices_setting_field(
            FAMILY_RATINGS,
            "Family Content Ratings",
            "The Content Ratings Family night shows. A title without one isn't shown.",
            RATINGS.map(|rating| (rating, rating)),
        )
    };
    Manifest {
        name: "Top Picks".to_owned(),
        description: "Shows each User titles they haven't started, ranked by the genres they \
                      play most and their Watchlist, in Top picks and Family night rows on \
                      home and at the top of it."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::HOME_ROW)],
        permissions: vec![
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_WATCHLISTS.into(),
        ],
        settings: vec![family_ratings],
        ..nama_pdk::manifest("top-picks", env!("CARGO_PKG_VERSION"))
    }
}

/// Ranks each User's titles as they open home.
struct TopPicks;

impl HomeRowSource for TopPicks {
    fn user_rows(asked: UserRowsAsked) -> Result<HomeRows, Error> {
        let family = nama_pdk::setting_picked(FAMILY_RATINGS)?;
        Ok(rows(&picks_for(asked.user_id)?, &family))
    }

    fn user_featured(asked: UserFeaturedAsked) -> Result<FeaturedSuggestions, Error> {
        Ok(featured(&picks_for(asked.user_id)?))
    }
}

nama_pdk::home_row_source!(TopPicks, [user_rows, user_featured]);

/// The User `user`'s picks of every title in every Library.
fn picks_for(user: i64) -> Result<Vec<Pick>, Error> {
    let mut titles = Vec::new();
    for library in kernel::libraries()? {
        titles.extend(kernel::titles(library.id)?);
    }
    // One per title, even one without a `title_ref`, so `ranked` pairs each with its own.
    let refs: Vec<_> = (titles.iter())
        .map(|title| (*title.title_ref).clone())
        .collect();
    // ponytail: one call for every title on the Server; ask a page at a time if a large Server's
    // answer outgrows the Plugin's memory.
    let states = kernel::watch_states(&refs, Some(user))?;
    // The Admin may grant the other Plugin Permissions alone (ADR 0071): rank by genres then.
    let watchlist = kernel::watchlist(user).unwrap_or_else(|err| {
        nama_pdk::warn!("ranking without the User's Watchlist: {err}");
        Vec::new()
    });
    Ok(ranked(&titles, &states, &watchlist))
}

/// A Movie, or a Show by its Episodes' ids, as a row and Featured name it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    kind: TitleKind,
    namespace: String,
    value: String,
}

impl Key {
    fn of(kind: TitleKind, id: &TitleId) -> Self {
        Self {
            kind,
            namespace: id.namespace.clone(),
            value: id.value.clone(),
        }
    }

    /// Every id of the Movie `title` is, or of the Show whose Episode it is, with the one the
    /// Kernel lists it under first; none for an Episode without its Show's id.
    fn every_of_title(title: &Title) -> Vec<Self> {
        let Some(title_ref) = title.title_ref.as_option() else {
            return Vec::new();
        };
        let (kind, preferred, others) = match title_ref.kind.as_known() {
            Some(TitleRefKind::TITLE_REF_KIND_MOVIE) => {
                (TitleKind::TITLE_KIND_MOVIE, &title_ref.id, &title.ids)
            }
            Some(TitleRefKind::TITLE_REF_KIND_EPISODE) => {
                (TitleKind::TITLE_KIND_SHOW, &title.show_id, &title.show_ids)
            }
            Some(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | None => return Vec::new(),
        };
        let Some(preferred) = preferred.as_option() else {
            return Vec::new();
        };
        (std::iter::once(preferred).chain(others))
            .map(|id| Self::of(kind, id))
            .collect()
    }

    fn id(&self) -> TitleId {
        TitleId {
            namespace: self.namespace.clone(),
            value: self.value.clone(),
            ..TitleId::default()
        }
    }
}

/// A Movie or Show the User hasn't started, with what ranks it.
#[derive(Debug)]
struct Pick {
    key: Key,
    content_rating: Option<String>,
    on_watchlist: bool,
    /// How many times the User played a title of each of its genres, summed.
    genre_plays: usize,
}

impl Pick {
    /// Whether the User has a reason to see it rather than only not having started it.
    const fn is_scored(&self) -> bool {
        self.on_watchlist || self.genre_plays > 0
    }
}

/// A Movie or Show on the Server: its genres and Content Rating, and whether the User started or
/// played it, or any of the Show's Episodes.
#[derive(Default)]
struct Candidate {
    /// Every id of the Movie, or of the Show by any of its Episodes, any of which a Watchlist
    /// may name it by (ADR 0042).
    ids: HashSet<Key>,
    genres: Vec<String>,
    content_rating: Option<String>,
    started: bool,
    played: bool,
}

/// The Movies and Shows of `titles` the User hasn't started or played, by `states`, their Watch
/// States, ranked: those on their `watchlist` first, then by how often they played the titles'
/// genres, then in the Kernel's order. A title whose Watch State isn't paired with it is left
/// out, so a title the User started never shows.
///
/// ponytail: a genre count and a Watchlist flag; weigh recent plays, or ratings, when Users ask
/// for better picks.
fn ranked(
    titles: &[Title],
    states: &[TitleWatchStates],
    watchlist: &[WatchlistTitle],
) -> Vec<Pick> {
    let mut order = Vec::new();
    let mut candidates: HashMap<Key, Candidate> = HashMap::new();
    for (title, state) in titles.iter().zip(states) {
        let ids = Key::every_of_title(title);
        let Some(key) = ids.first().filter(|_| state.title_ref == title.title_ref) else {
            continue;
        };
        let candidate = candidates.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Candidate {
                genres: title.genres.clone(),
                content_rating: title.content_rating.clone(),
                ..Candidate::default()
            }
        });
        candidate.ids.extend(ids);
        candidate.started |= (state.states.iter()).any(|it| it.played || it.position_ms > 0);
        candidate.played |= (state.states.iter()).any(|it| it.played);
    }
    let mut genre_plays: HashMap<&str, usize> = HashMap::new();
    for genre in (candidates.values().filter(|title| title.played)).flat_map(|title| &title.genres)
    {
        *genre_plays.entry(genre).or_default() += 1;
    }
    let watchlist: HashSet<Key> = (watchlist.iter())
        .filter_map(|title| Some(Key::of(title.kind.as_known()?, title.id.as_option()?)))
        .collect();
    let mut picks: Vec<Pick> = (order.into_iter())
        .filter_map(|key| {
            let candidate = candidates
                .get(&key)
                .filter(|candidate| !candidate.started)?;
            Some(Pick {
                on_watchlist: !candidate.ids.is_disjoint(&watchlist),
                genre_plays: (candidate.genres.iter())
                    .map(|genre| genre_plays.get(genre.as_str()).copied().unwrap_or_default())
                    .sum(),
                content_rating: candidate.content_rating.clone(),
                key,
            })
        })
        .collect();
    picks.sort_by_key(|pick| std::cmp::Reverse((pick.on_watchlist, pick.genre_plays)));
    picks
}

/// The "Top picks" row of every pick and the "Family night" row of those rated one of
/// `family_ratings`, ignoring case.
fn rows(picks: &[Pick], family_ratings: &[impl AsRef<str>]) -> HomeRows {
    let is_family = |pick: &&Pick| {
        (pick.content_rating.as_deref()).is_some_and(|rating| {
            (family_ratings.iter()).any(|family| family.as_ref().eq_ignore_ascii_case(rating))
        })
    };
    HomeRows {
        rows: vec![
            row("top-picks", "Top picks", picks.iter()),
            row(
                "family-night",
                "Family night",
                picks.iter().filter(is_family),
            ),
        ],
        ..HomeRows::default()
    }
}

fn row<'a>(key: &str, heading: &str, picks: impl Iterator<Item = &'a Pick>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: heading.to_owned(),
        ttl_seconds: FRESH_FOR_SECONDS,
        titles: picks
            .map(|pick| HomeRowTitle {
                kind: pick.key.kind.into(),
                id: pick.key.id().into(),
                ..HomeRowTitle::default()
            })
            .collect(),
        ..HomeRow::default()
    }
}

/// The first [`FEATURED_PICKS`] of `picks` the User has a reason to see, heaviest first.
fn featured(picks: &[Pick]) -> FeaturedSuggestions {
    let weights = std::iter::successors(Some(FEATURED_TOP_WEIGHT), |weight| {
        Some(weight - FEATURED_STEP)
    });
    FeaturedSuggestions {
        suggestions: (picks.iter().filter(|pick| pick.is_scored()))
            .take(FEATURED_PICKS)
            .zip(weights)
            .map(|(pick, weight)| FeaturedSuggestion {
                kind: pick.key.kind.into(),
                id: pick.key.id().into(),
                weight,
                ..FeaturedSuggestion::default()
            })
            .collect(),
        ttl_seconds: FRESH_FOR_SECONDS,
        ..FeaturedSuggestions::default()
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
