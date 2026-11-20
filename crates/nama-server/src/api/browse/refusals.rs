//! What a client gets when Browse or Home rows find nothing or fail.

use connectrpc::ConnectError;
use nama_proto::nama::v1::RefusalReason;

use crate::{
    api::{Refused, database_failed, refuse},
    browse, home_rows,
    library::{LibraryKind, NotFound},
};

/// The request's fields naming what Browse looks up, so the refusal for one it didn't find names
/// its field. A request without one has `""`, and its refusal no field.
#[derive(Clone, Copy)]
pub(in crate::api) struct Fields {
    pub(in crate::api) library: &'static str,
    /// The Movie's, Show's or Episode's id; a Season's Show's.
    pub(in crate::api) title: &'static str,
    pub(in crate::api) season: &'static str,
    /// An Episode's number within its Season.
    pub(in crate::api) episode: &'static str,
    pub(in crate::api) version: &'static str,
}

impl Fields {
    /// For a call that looks nothing up by id.
    pub(in crate::api) const NONE: Self = Self {
        library: "",
        title: "",
        season: "",
        episode: "",
        version: "",
    };
    /// A Browse request naming only its Library, in `library_id`.
    pub(in crate::api) const LIBRARY: Self = Self {
        library: "library_id",
        ..Self::NONE
    };
}

/// What makes the Connect error a client gets for a Library, title, Season or Version that
/// Browse didn't find, naming its field in `fields`. Database failures are logged, not sent.
pub(in crate::api) fn refused(fields: Fields) -> impl Fn(browse::Error) -> ConnectError {
    move |err| match err {
        browse::Error::NotFound(err) => not_found(fields, &err).into(),
        browse::Error::PageTokenInvalid(_) => refuse(
            RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
            err.to_string(),
        )
        .field("page.token")
        .into(),
        browse::Error::Database(err) => database_failed(err),
    }
}

/// The refusal for finding nothing of what `fields` name.
pub(in crate::api) fn not_found(fields: Fields, err: &NotFound) -> Refused {
    use RefusalReason as R;
    let message = err.to_string();
    match *err {
        NotFound::NoSuchLibrary(id) => refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message)
            .field(fields.library)
            .param("libraryId", &id),
        NotFound::NothingMatched => {
            refuse(R::REFUSAL_REASON_NOTHING_MATCHED, message).field(fields.title)
        }
        NotFound::NoSuchSeason => {
            refuse(R::REFUSAL_REASON_SEASON_NOT_FOUND, message).field(fields.season)
        }
        NotFound::NoSuchEpisode => {
            refuse(R::REFUSAL_REASON_EPISODE_NOT_FOUND, message).field(fields.episode)
        }
        NotFound::NoSuchVersion => {
            refuse(R::REFUSAL_REASON_VERSION_NOT_FOUND, message).field(fields.version)
        }
        NotFound::WrongKind { wanted } => {
            let kind = match wanted {
                LibraryKind::Movies => "movies",
                LibraryKind::Shows => "shows",
            };
            refuse(R::REFUSAL_REASON_WRONG_LIBRARY_KIND, message)
                .field(fields.library)
                .param("libraryKind", &kind)
        }
    }
}

/// The Connect error a client gets when Home rows couldn't be read. Database failures are logged,
/// not sent.
pub(super) fn home_rows_refused(err: home_rows::Error) -> ConnectError {
    match err {
        home_rows::Error::PageTokenInvalid(_) => refuse(
            RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
            err.to_string(),
        )
        .field("page.token")
        .into(),
        home_rows::Error::Database(err) => database_failed(err),
        home_rows::Error::Libraries(err) => err.into(),
    }
}
