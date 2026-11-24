//! The versions stored with each file and Match, so the next scan reads again what an older
//! Server read. When to bump [`PARSER_VERSION`] is in `src/scanner/AGENTS.md`, and when to bump
//! [`METADATA_VERSION`] is in `src/matching/AGENTS.md`.

/// The version of the name parser in `src/scanner/names.rs`, stored with each file it reads. See
/// `src/scanner/AGENTS.md` for when to bump it.
pub(crate) const PARSER_VERSION: i64 = 11;

/// The version of what a Match stores from the title sources, stored with each Match. See
/// `src/matching/AGENTS.md` for when to bump it. Version 2 adds artwork picked across the
/// Library's sources, Title Logos among it; version 3 genres, Content Ratings and a Show's own
/// Poster, overview and first air date; version 4 picks a Backdrop without text first; version 5
/// adds a titled Backdrop; version 6 a title's Score and original language; version 7 every
/// Season's Episode air dates; version 8 a title's Trailers; version 9 its Roles; version 10 its
/// Similar titles; version 11 a Movie's Box Set.
pub(crate) const METADATA_VERSION: i64 = 11;
