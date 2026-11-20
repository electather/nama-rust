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
}

impl HomeRowId {
    /// The Plugin giving the row, or `None` for one the Kernel builds.
    pub(crate) fn plugin(&self) -> Option<&PluginId> {
        match self {
            Self::ContinueWatching => None,
            Self::Shared { plugin, .. } | Self::UsersOwn { plugin } => Some(plugin),
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
                let found = (self.rows.iter().enumerate()).find(|(_, placed)| placed.row == row.0);
                let (place, shown) = found.map_or((self.rows.len(), true), |(place, placed)| {
                    (place, placed.shown)
                });
                (place, row, shown)
            })
            .collect();
        placed.sort_by_key(|&(place, ..)| place);
        placed
            .into_iter()
            .map(|(_, row, shown)| (row, shown))
            .collect()
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
