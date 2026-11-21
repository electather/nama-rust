//! The default Home layout: the order of the Home rows every User's Home follows, and which of
//! them and whether Featured are shown (ADR 0077).

use nama_plugin_api::PluginId;
use serde::{Deserialize, Serialize};

/// A Home layout, stored as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HomeLayout {
    pub(crate) featured_shown: bool,
    /// In the order Home shows them.
    pub(crate) rows: Vec<PlacedRow>,
}

impl Default for HomeLayout {
    fn default() -> Self {
        Self {
            featured_shown: true,
            rows: Vec::new(),
        }
    }
}

/// A row's place in a [`HomeLayout`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PlacedRow {
    pub(crate) row: HomeRowId,
    pub(crate) shown: bool,
}

/// What a [`HomeLayout`] places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HomeRowId {
    ContinueWatching,
    /// A row the Plugin gives every User.
    Shared {
        plugin: PluginId,
        key: String,
    },
    /// The rows the Plugin gives each User for them alone, together.
    UsersOwn {
        plugin: PluginId,
    },
    /// One of the rows the Plugin gives one User for them alone, which the default places as
    /// [`Self::UsersOwn`].
    Own {
        plugin: PluginId,
        key: String,
    },
}

impl HomeRowId {
    /// The Plugin giving the row, or `None` for one the Kernel builds.
    pub(crate) fn plugin(&self) -> Option<&PluginId> {
        match self {
            Self::ContinueWatching => None,
            Self::Shared { plugin, .. } | Self::UsersOwn { plugin } | Self::Own { plugin, .. } => {
                Some(plugin)
            }
        }
    }

    /// Whether a client naming this in a User's Home layout names `row`: the same row, or for
    /// one of the User's own rows, its Plugin's row of the same key, as the API gives it.
    pub(crate) fn names(&self, row: &Self) -> bool {
        match (self, row) {
            (
                Self::Shared { plugin, key },
                Self::Own {
                    plugin: of,
                    key: its,
                },
            ) => plugin == of && key == its,
            _ => self == row,
        }
    }

    /// Whether a layout placing this places `row`: the same row, or one User's own row of the
    /// Plugin whose entry for them this is.
    fn places(&self, row: &Self) -> bool {
        match (self, row) {
            (Self::UsersOwn { plugin }, Self::Own { plugin: of, .. }) => plugin == of,
            _ => self == row,
        }
    }
}

impl HomeLayout {
    /// `rows` in this layout's order, each with whether it is shown: those it places at their
    /// first place, then those it doesn't, shown. Rows of one place, and those it doesn't place,
    /// keep the order given.
    pub(crate) fn arrange<T>(&self, rows: Vec<(HomeRowId, T)>) -> Vec<((HomeRowId, T), bool)> {
        let mut placed: Vec<_> = (rows.into_iter())
            .map(|row| {
                let (place, shown) = self.place_of(&row.0);
                (place, row, shown)
            })
            .collect();
        placed.sort_by_key(|&(place, ..)| place);
        placed
            .into_iter()
            .map(|(_, row, shown)| (row, shown))
            .collect()
    }

    /// Whether this layout shows `row`: as its first place does, or shown when it has none.
    pub(crate) fn shows(&self, row: &HomeRowId) -> bool {
        self.place_of(row).1
    }

    /// Where `row` first is in this layout, with whether it is shown there, or past the end, shown.
    fn place_of(&self, row: &HomeRowId) -> (usize, bool) {
        let found = (self.rows.iter().enumerate()).find(|(_, placed)| placed.row.places(row));
        found.map_or((self.rows.len(), true), |(place, placed)| {
            (place, placed.shown)
        })
    }

    /// This layout with each row `before` places that it doesn't, such as a disabled Plugin's,
    /// which the editor didn't see: each after the row it followed in `before`, or first.
    pub(super) fn keeping_unnamed(mut self, before: &Self) -> Self {
        let mut at = 0;
        for placed in &before.rows {
            if let Some(place) = self.rows.iter().position(|named| named.row == placed.row) {
                at = place + 1;
            } else {
                self.rows.insert(at, placed.clone());
                at += 1;
            }
        }
        self
    }

    /// Drops the rows of the Plugins `gone` names.
    pub(super) fn forget(&mut self, gone: &[PluginId]) {
        (self.rows).retain(|placed| {
            placed
                .row
                .plugin()
                .is_none_or(|plugin| !gone.contains(plugin))
        });
    }
}
