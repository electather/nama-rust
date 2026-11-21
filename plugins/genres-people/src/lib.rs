//! The Genres and People Installed Plugin: a Similar source suggesting the titles in the Libraries
//! that share a title's Persons and genres (ADR 0084), read through the Kernel's
//! `list_titles_sharing`. The Server builds it for wasm32 and bundles it in its binary.

use std::collections::{HashMap, HashSet};

use nama_pdk::{
    Manifest, Permission, SharingTitle, SimilarAsked, SimilarSource, SimilarTitle, SimilarTitles,
    TitleId, TitleRole, extism_pdk::Error, kernel, services,
};

/// The Crew jobs whose shared Person weighs [`KEY_CREW_WEIGHT`]: a Director, Creator or Writer,
/// whom TMDB also lists under the job `Screenplay`.
const KEY_JOBS: [&str; 4] = ["Director", "Creator", "Writer", "Screenplay"];
const KEY_CREW_WEIGHT: f32 = 0.3;
/// How many of the Cast, first billed, count; a shared one weighs up to [`CAST_WEIGHT`], less the
/// further down the billing either title has them.
const TOP_CAST: u8 = 10;
const CAST_WEIGHT: f32 = 0.1;
/// What sharing every genre of both titles weighs.
const GENRES_WEIGHT: f32 = 0.2;
/// The most a title weighs, so a title source's top pick, at 1.0, still leads the row.
const MAX_WEIGHT: f32 = 0.9;
/// How many titles to ask the Kernel for: its most.
const CANDIDATES: u32 = 1000;

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
    Manifest {
        name: "Genres and People".to_owned(),
        description: "Adds the titles in your Libraries that share a title's director, creator, \
                      writer, main cast or genres to its Similar titles."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::SIMILAR_SOURCE)],
        permissions: vec![Permission::PERMISSION_READ_LIBRARIES.into()],
        ..nama_pdk::manifest("genres-people", env!("CARGO_PKG_VERSION"))
    }
}

/// Weighs the Libraries' titles sharing a title's Persons and genres.
struct GenresPeople;

impl SimilarSource for GenresPeople {
    fn similar(asked: SimilarAsked) -> Result<SimilarTitles, Error> {
        let Some(kind) = asked.kind.as_known() else {
            return Ok(SimilarTitles::default());
        };
        let title = Profile::of(&asked.roles, &asked.genres);
        let people: Vec<TitleId> = (title.people.iter())
            .map(|&(namespace, value)| TitleId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
                ..TitleId::default()
            })
            .collect();
        let sharing = kernel::titles_sharing(kind, &people, &asked.genres, CANDIDATES)?;
        let mut titles: Vec<SimilarTitle> = (sharing.iter())
            .filter(|candidate| !is_asked(&asked.ids, candidate))
            .filter_map(|candidate| {
                let weight = title.weigh(&Profile::of(&candidate.roles, &candidate.genres))?;
                Some(SimilarTitle {
                    id: candidate.ids.first()?.clone().into(),
                    weight,
                    ..SimilarTitle::default()
                })
            })
            .collect();
        titles.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        Ok(SimilarTitles {
            titles,
            ..SimilarTitles::default()
        })
    }
}

nama_pdk::similar_source!(GenresPeople);

/// Whether `candidate` is the title asked about, known by any of `ids`.
fn is_asked(ids: &[TitleId], candidate: &SharingTitle) -> bool {
    (candidate.ids.iter()).any(|id| ids.iter().any(|asked| asked == id))
}

/// A Person, by the namespace and value of their External ID.
type Person<'a> = (&'a str, &'a str);

/// What a title is weighed by: its Persons and genres.
struct Profile<'a> {
    people: HashSet<Person<'a>>,
    key_crew: HashSet<Person<'a>>,
    /// Each of the first [`TOP_CAST`] of the Cast, by their first place in its billing.
    top_cast: HashMap<Person<'a>, u8>,
    genres: HashSet<&'a str>,
}

impl<'a> Profile<'a> {
    fn of(roles: &'a [TitleRole], genres: &'a [String]) -> Self {
        let mut profile = Self {
            people: HashSet::new(),
            key_crew: HashSet::new(),
            top_cast: HashMap::new(),
            genres: genres.iter().map(String::as_str).collect(),
        };
        for role in roles {
            let Some(id) = role.person.as_option() else {
                continue;
            };
            let person = (id.namespace.as_str(), id.value.as_str());
            profile.people.insert(person);
            if role.crew && KEY_JOBS.contains(&role.part.as_str()) {
                profile.key_crew.insert(person);
            }
            let top_billed = u8::try_from(role.billed)
                .ok()
                .filter(|&billed| billed < TOP_CAST);
            if let Some(billed) = top_billed.filter(|_| !role.crew) {
                let first = profile.top_cast.entry(person).or_insert(billed);
                *first = (*first).min(billed);
            }
        }
        profile
    }

    /// How alike `other` is to this title, up to [`MAX_WEIGHT`], or `None` when it shares no
    /// Person and less than half its own genres.
    fn weigh(&self, other: &Self) -> Option<f32> {
        let shared_genres = self.genres.intersection(&other.genres).count();
        let half_its_genres = shared_genres > 0 && shared_genres * 2 >= other.genres.len();
        if self.people.is_disjoint(&other.people) && !half_its_genres {
            return None;
        }
        let key_crew =
            KEY_CREW_WEIGHT * counted(self.key_crew.intersection(&other.key_crew).count());
        let cast: f32 = (self.top_cast.iter())
            .filter_map(|(person, &billed)| Some(billed.max(*other.top_cast.get(person)?)))
            .map(|worse| CAST_WEIGHT * f32::from(TOP_CAST - worse) / f32::from(TOP_CAST))
            .sum();
        let all_genres = self.genres.union(&other.genres).count();
        let genres = GENRES_WEIGHT * counted(shared_genres) / counted(all_genres.max(1));
        Some((key_crew + cast + genres).min(MAX_WEIGHT))
    }
}

/// `count` as a weight multiplies it.
#[expect(
    clippy::cast_precision_loss,
    reason = "a title's Roles and genres number far fewer than f32 holds exactly"
)]
const fn counted(count: usize) -> f32 {
    count as f32
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
