//! What a Library holds: the video files a scan stored, the Items their names say they hold, what
//! Matching decided about each Item, and the Matches an admin pinned.
//!
//! [`Contents`] owns the tables (see `src/library/AGENTS.md`); each method is one read or one write.
//! A write that stores Matches also makes the Matched Movies' and Seasons' Requests Available, or
//! Partially Available (ADR 0012).

use std::sync::Arc;

use crate::db::Database;

mod decisions;
mod files;
#[cfg(test)]
pub(crate) mod fixture;
mod ids;
mod items;
mod matched;
mod pins;
mod search;
#[cfg(test)]
mod testing;
#[cfg(test)]
pub(crate) use testing::{File, FileItem, Recorded};

use decisions::insert_decisions;
pub(crate) use decisions::{
    Decided, Decisions, ImageRef, Metadata, TiedTitle, Unmatched, UnmatchedReason,
};
pub(crate) use files::{Change, Stored};
pub(crate) use ids::{DEFAULT_ORDER, parse_hint, primary_id};
use ids::{ids_from_json, ids_sql, primary_id_in};
use items::ItemRow;
pub(crate) use items::{Item, ItemKind, Name};
pub(crate) use matched::{
    Episode, MatchedTo, Movie, NewestBackdrop, Season, Show, Version, library_with_movie,
    matched_episodes, show_poster,
};
pub(crate) use pins::{Named, Pin};
pub(crate) use search::{Found, FoundEpisode, FoundMovie, FoundShow};

/// Reads and writes the contents of Libraries.
#[derive(Debug, Clone)]
pub(crate) struct Contents {
    db: Arc<Database>,
}

impl Contents {
    /// The contents of the Libraries in `db`.
    #[must_use]
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}
