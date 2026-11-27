//! Where a title is for a User: the one answer Browse, Home rows and Requests share when Libraries
//! hold the same title (`GLOSSARY.md`, "Library").

use std::collections::{HashMap, HashSet};

use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

use super::{Contents, Library, LibraryKind, OnServer, TitleKind, contents::copies_of};
use crate::users::Reach;

/// Where a title is for a User, given their [`Reach`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Whereabouts<T = OnServer> {
    /// In the oldest Library they see that holds it, whose copy's Content Rating reaches them.
    In(T),
    /// On the Server but kept from them: only Libraries they don't see hold it, or their Maximum
    /// Age keeps out the copy in the oldest one they see.
    KeptFrom,
    /// In no Library.
    Nowhere,
}

impl<T> Whereabouts<T> {
    /// What it is in, if it reaches them.
    pub(crate) fn reached(self) -> Option<T> {
        match self {
            Self::In(title) => Some(title),
            Self::KeptFrom | Self::Nowhere => None,
        }
    }
}

impl Whereabouts {
    /// Where each title of `kind` that `ids` name is in `libraries`, oldest first, for a User whose
    /// Reach is `reach`, under each of its ids among `ids`. An id no Library holds is left out:
    /// that title is [`Whereabouts::Nowhere`].
    pub(crate) async fn of_titles(
        contents: &Contents,
        libraries: &[Library],
        reach: &Reach,
        kind: LibraryKind,
        ids: &[ExternalId],
    ) -> Result<HashMap<ExternalId, Self>, sqlx::Error> {
        let mut libraries = libraries.to_vec();
        // `find` takes each title from the first Library that has it, so those seen go first.
        libraries.sort_by_key(|library| !reach.sees(library.id));
        let found = OnServer::find(contents, &libraries, kind, ids).await?;
        Ok((found.into_iter())
            .map(|(id, title)| (id, placed(reach, [title], held_in)))
            .collect())
    }

    /// The titles of `kind` that `ids` name and are [`Whereabouts::In`] a Library for `reach`, as
    /// [`Self::of_titles`] finds them, in the order of `ids`, a title named by two of them once, at
    /// the first.
    pub(crate) async fn reached_in_order(
        contents: &Contents,
        libraries: &[Library],
        reach: &Reach,
        kind: LibraryKind,
        ids: &[ExternalId],
    ) -> Result<Vec<OnServer>, sqlx::Error> {
        let mut found = Self::of_titles(contents, libraries, reach, kind, ids).await?;
        let mut shown = HashSet::new();
        Ok((ids.iter())
            .filter_map(|id| found.remove(id)?.reached())
            .filter(|title| shown.insert(title.id().clone()))
            .collect())
    }
}

impl Whereabouts<i64> {
    /// Where the title of `kind` known by `id` is for a User whose Reach is `reach`, by the id of
    /// the Library it is in. Read on `conn`, so a write can act on it before a scan changes it.
    pub(crate) async fn of_title(
        conn: &mut SqliteConnection,
        reach: &Reach,
        kind: TitleKind,
        id: &ExternalId,
    ) -> Result<Self, sqlx::Error> {
        let copies = copies_of(conn, kind, id).await?;
        Ok(
            match placed(reach, copies, |(library, rating)| {
                (*library, rating.as_deref())
            }) {
                Whereabouts::In((library, _)) => Self::In(library),
                Whereabouts::KeptFrom => Self::KeptFrom,
                Whereabouts::Nowhere => Self::Nowhere,
            },
        )
    }
}

/// The rule: where a title whose `copies`, oldest Library first, are each held in the Library and
/// rated as `held` reads them, is for a User whose Reach is `reach`.
fn placed<T>(
    reach: &Reach,
    copies: impl IntoIterator<Item = T>,
    held: impl Fn(&T) -> (i64, Option<&str>),
) -> Whereabouts<T> {
    let mut copies = copies.into_iter().peekable();
    if copies.peek().is_none() {
        return Whereabouts::Nowhere;
    }
    let Some(seen) = copies.find(|copy| reach.sees(held(copy).0)) else {
        return Whereabouts::KeptFrom;
    };
    let (library, rating) = held(&seen);
    if reach.reaches(library, rating) {
        Whereabouts::In(seen)
    } else {
        Whereabouts::KeptFrom
    }
}

fn held_in(title: &OnServer) -> (i64, Option<&str>) {
    (title.library(), title.content_rating())
}

#[cfg(test)]
#[path = "whereabouts_tests.rs"]
mod tests;
