//! What a file holds, by its name: its Items and the names they go by.

use std::borrow::Cow;

use nama_plugin_api::metadata::ExternalId;

use crate::library::title_source::TitleKind;

use super::parse_hint;

/// Something a file holds, as far as its path says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    /// A Movie.
    Movie {
        /// The title, as the folder or file names it.
        title: String,
        /// The release year.
        year: Option<i64>,
        /// The Movie's id, from an ID Hint.
        hint: Option<ExternalId>,
    },
    /// One Episode of a Show.
    Episode {
        /// The Show's title, as its folder names it.
        show: String,
        /// The year the Show first aired.
        year: Option<i64>,
        /// The Show's id, from an ID Hint on its folder.
        hint: Option<ExternalId>,
        /// The Season number; 0 holds specials.
        season: i64,
        /// The Episode number within the Season.
        episode: i64,
    },
}

/// The name an Item goes by: a Movie's title, year and ID hint, or those of the Show an Episode
/// is in. Items of a Library named alike share a pin.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Name {
    pub(super) kind: ItemKind,
    pub(crate) title: String,
    pub(crate) year: Option<i64>,
    pub(crate) hint: Option<ExternalId>,
}

impl Item {
    pub(crate) fn name(&self) -> Name {
        let (Self::Movie { hint, .. } | Self::Episode { hint, .. }) = self;
        let row = ItemRow::from(self);
        Name {
            kind: row.kind,
            title: row.title.into_owned(),
            year: row.year,
            hint: hint.clone(),
        }
    }
}

impl Name {
    pub(crate) const fn kind(&self) -> TitleKind {
        match self.kind {
            ItemKind::Movie => TitleKind::Movie,
            ItemKind::Episode => TitleKind::Show,
        }
    }
}

/// Whether an Item is a Movie or an Episode, as `library_items.kind` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum ItemKind {
    Movie,
    Episode,
}

#[derive(Debug)]
pub(super) struct ItemRow<'a> {
    pub(super) kind: ItemKind,
    pub(super) title: Cow<'a, str>,
    pub(super) year: Option<i64>,
    /// An [`ExternalId`] as it displays, such as `tvdb-433631`.
    pub(super) hint: Option<String>,
    pub(super) season: Option<i64>,
    pub(super) episode: Option<i64>,
}

impl ItemRow<'_> {
    /// The Item the row holds. `None` for an Episode without its numbers, which the schema
    /// forbids.
    pub(super) fn into_item(self) -> Option<Item> {
        let Self {
            kind,
            title,
            year,
            hint,
            season,
            episode,
        } = self;
        let hint = hint.as_deref().and_then(parse_hint);
        Some(match kind {
            ItemKind::Movie => Item::Movie {
                title: title.into_owned(),
                year,
                hint,
            },
            ItemKind::Episode => Item::Episode {
                show: title.into_owned(),
                year,
                hint,
                season: season?,
                episode: episode?,
            },
        })
    }
}

impl<'a> From<&'a Item> for ItemRow<'a> {
    fn from(item: &'a Item) -> Self {
        match *item {
            Item::Movie {
                ref title,
                year,
                ref hint,
            } => Self {
                kind: ItemKind::Movie,
                title: Cow::Borrowed(title),
                year,
                hint: hint.as_ref().map(ExternalId::to_string),
                season: None,
                episode: None,
            },
            Item::Episode {
                ref show,
                year,
                ref hint,
                season,
                episode,
            } => Self {
                kind: ItemKind::Episode,
                title: Cow::Borrowed(show),
                year,
                hint: hint.as_ref().map(ExternalId::to_string),
                season: Some(season),
                episode: Some(episode),
            },
        }
    }
}
