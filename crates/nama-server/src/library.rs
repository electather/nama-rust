//! The Library area: what a Library holds, its title sources and Recognisers, artwork, Custom
//! Images and Watch State. Its rules are in `src/library/AGENTS.md`.

mod artwork;
mod contents;
mod country_tags;
mod custom_images;
mod folders;
mod last_scans;
mod libraries;
mod local_images;
mod recognisers;
mod scan_locks;
mod shown_images;
mod similar_sources;
mod title_source;
mod titles;
mod versions;
mod watch_states;

pub use artwork::{Artwork, Error as ArtworkError};
pub(crate) use artwork::{Picked, router as artwork_router};
pub(crate) use contents::{
    Change, Contents, DEFAULT_ORDER, Decided, Decisions, Episode as LibraryEpisode, Found,
    FoundEpisode, FoundMovie, FoundShow, ImageRef, Item, ItemKind, Listing, LocalImageFile,
    LocalRole, MatchedTo, Metadata, Movie as LibraryMovie, Name, Named, NewestBackdrop, NfoFile,
    NfoHints, NfoItem, PersonRole, Pin, Season as LibrarySeason, Show as LibraryShow, Stored,
    StoredBoxSet, TiedTitle, TitleSort, TitlesIn, Unmatched, UnmatchedReason,
    Version as LibraryVersion, Watched, library_with_movie, library_with_show, match_bands,
    matched_episodes, matched_ids, primary_id,
};
#[cfg(test)]
pub(crate) use contents::{File, FileItem, Recorded, fixture};
pub(crate) use country_tags::country_tag;
pub(crate) use custom_images::{CustomImages, Role, Slot, router as custom_images_router};
pub(crate) use folders::FolderProblem;
pub(crate) use last_scans::LastScan;
pub(crate) use libraries::{
    Error as LibraryError, Kind as LibraryKind, Libraries, Library, Order, check_known,
    default_title_sources,
};
pub(crate) use local_images::{FoundImages, LocalImages, router as local_images_router};
pub(crate) use recognisers::{RecogniserFailed, Recognisers};
pub(crate) use scan_locks::{ScanLock, ScanLocks};
pub(crate) use shown_images::{Downloaded, ImageSources, ShownImages, TitleImage};
pub(crate) use similar_sources::{
    Answer as SimilarAnswer, MAX_SIMILAR, SimilarSources, merged as merged_similar,
};
pub(crate) use title_source::{
    Candidate, Episode as SourceEpisode, ListedMovie, Roles, SearchQuery, Season as SourceSeason,
    Show as SourceShow, Source, TitleKind, TitleSources, Trailer, Unanswered, title_roles,
};
#[cfg(test)]
pub(crate) use title_source::{Role as SourceRole, scripted};
pub(crate) use titles::{
    NotFound, OnServer, Target as TitleTarget, Titles, episode_name, known_as, named_episodes,
};
pub(crate) use versions::{METADATA_VERSION, PARSER_VERSION};
pub(crate) use watch_states::{
    Kind as WatchStateKind, MovieOrEpisode, Progress, WatchState, WatchStates,
    Watched as WatchedTitle,
};
