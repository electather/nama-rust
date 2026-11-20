//! The Library area: what a Library holds, its title sources and Recognisers, artwork, Custom
//! Images and Watch State. Its rules are in `src/library/AGENTS.md`.

mod artwork;
mod contents;
mod country_tags;
mod custom_images;
mod libraries;
mod local_images;
mod recognisers;
mod scan_locks;
mod title_source;
mod titles;
mod versions;
mod watch_states;

pub use artwork::{Artwork, Error as ArtworkError};
pub(crate) use artwork::{Picked, router as artwork_router, url as artwork_url};
pub(crate) use contents::{
    Change, Contents, DEFAULT_ORDER, Decided, Decisions, Episode as LibraryEpisode, Found,
    FoundEpisode, FoundMovie, FoundShow, ImageRef, Item, ItemKind, Listing, LocalImageFile,
    LocalRole, MatchedTo, Metadata, Movie as LibraryMovie, Name, Named, NewestBackdrop, Pin,
    Season as LibrarySeason, Show as LibraryShow, Stored, TiedTitle, TitleSort, TitlesIn,
    Unmatched, UnmatchedReason, Version as LibraryVersion, Watched, library_with_movie,
    matched_episodes, primary_id,
};
#[cfg(test)]
pub(crate) use contents::{File, FileItem, Recorded, fixture};
pub(crate) use country_tags::country_tag;
pub(crate) use custom_images::{
    CustomImages, Role, Slot, Urls as CustomImageUrls, router as custom_images_router,
};
pub(crate) use libraries::{
    Error as LibraryError, FolderProblem, Kind as LibraryKind, Libraries, Library, Order,
    check_known, check_title_sources, default_title_sources,
};
pub(crate) use local_images::{
    FoundImages, LocalImageUrls, LocalImages, router as local_images_router,
};
pub(crate) use recognisers::{RecogniserFailed, Recognisers};
pub(crate) use scan_locks::{ScanLock, ScanLocks};
pub(crate) use title_source::{
    Candidate, Episode as SourceEpisode, SearchQuery, Season as SourceSeason, Show as SourceShow,
    Source, TitleKind, TitleSources, Unanswered,
};
pub(crate) use titles::{
    NotFound, Target as TitleTarget, Titles, episode_name, known_as, named_episodes,
};
pub(crate) use versions::PARSER_VERSION;
// Matching names it only in its docs, so outside rustdoc nothing would use it.
#[cfg(doc)]
pub(crate) use versions::METADATA_VERSION;
pub(crate) use watch_states::{
    Kind as WatchStateKind, MovieOrEpisode, Progress, WatchState, WatchStates,
    Watched as WatchedTitle,
};
