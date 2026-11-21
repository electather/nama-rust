//! What a Library holds: the video files a scan stored, the Items their names say they hold, what
//! Matching decided about each Item, and the Matches an admin pinned.
//!
//! [`Contents`] owns the tables (see `src/library/AGENTS.md`); each method is one read or one write.
//! A write that stores Matches also makes the Matched Movies' and Seasons' Requests Available, or
//! Partially Available (ADR 0012).

use std::sync::Arc;

use crate::db::Database;

mod air_dates;
mod backdrops;
mod decisions;
mod edit_distance;
mod files;
#[cfg(test)]
pub(crate) mod fixture;
mod ids;
mod items;
mod listed;
mod local_images;
mod match_images;
mod matched;
mod pages;
mod per_title;
mod pins;
mod search;
mod sort_titles;
#[cfg(test)]
mod testing;
mod titles_sql;
#[cfg(test)]
pub(crate) use testing::{File, FileItem, Recorded};

pub(crate) use backdrops::NewestBackdrop;
use decisions::insert_decisions;
pub(crate) use decisions::{
    Decided, Decisions, ImageRef, Metadata, TiedTitle, Unmatched, UnmatchedReason,
};
pub(crate) use files::{Change, Stored};
pub(crate) use ids::{DEFAULT_ORDER, matched_ids, parse_hint, primary_id};
use ids::{ids_from_json, ids_sql, primary_id_in};
use items::ItemRow;
pub(crate) use items::{Item, ItemKind, Name};
pub(crate) use local_images::{LocalImage, LocalImageFile, LocalRole, TitleLocalImage};
pub(crate) use matched::{
    Episode, MatchedTo, Movie, Season, Show, TitlesIn, Version, library_with_movie,
    library_with_show, matched_episodes, show_poster,
};
pub(crate) use pages::{Listing, TitleSort, Watched};
pub(crate) use pins::{Named, Pin};
pub(crate) use search::{Found, FoundEpisode, FoundMovie, FoundShow, match_bands};
use titles_sql::{library_titles, listed_id_sql, matched_items};

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
