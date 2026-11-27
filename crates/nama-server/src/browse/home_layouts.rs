//! A User's Home: the Home rows they could see, as their Home layout arranges and shows them, and
//! their Featured, and the default Home layout with every row Home could show (ADR 0077). Which
//! rows a User could see, and the Kernel's rows' order before the Plugins', are written here once.

use std::sync::Arc;

use super::{Browsing, Error, Featured};
use crate::{
    db::Database,
    home_rows::HomeRows,
    library::Library,
    server_settings::{HomeRowId, Settings},
    users::{AskedHomeLayout, User, UserHomeLayout},
};

/// A Home row: what a Home layout places it by, with what shows of it.
pub(crate) type HomeRow = (HomeRowId, ShownRow);

/// What shows of a Home row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShownRow {
    /// Continue Watching, which shows nothing beyond what places it.
    ContinueWatching,
    /// The newest titles of this Library.
    RecentlyAdded(Library),
    /// A Plugin's row, linked to the Library `library` only if its User sees that one.
    Plugin {
        /// What its Plugin names it by, as `ListHomeRowTitles` asks for it.
        key: String,
        heading: String,
        library: Option<i64>,
    },
    /// Each User's own rows of a Plugin, as one entry: only the default Home layout lists it, as a
    /// User's Home lists each of theirs.
    UsersOwn,
}

/// A Home layout as its editor sees it.
#[derive(Debug)]
pub(crate) struct SeenHomeLayout {
    /// Whether Featured shows, at the top of Home.
    pub(crate) featured_shown: bool,
    /// Every row in its order, each with whether it shows.
    pub(crate) rows: Vec<(HomeRow, bool)>,
}

/// Why a User's Home couldn't be read or saved.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HomeLayoutError {
    /// Listing the Plugins' rows or their Featured suggestions failed.
    #[error(transparent)]
    HomeRows(#[from] crate::home_rows::Error),
    /// Reading the Libraries, the titles or a Home layout failed.
    #[error(transparent)]
    Browse(#[from] Error),
}

impl From<sqlx::Error> for HomeLayoutError {
    fn from(err: sqlx::Error) -> Self {
        Self::Browse(err.into())
    }
}

/// Composes each User's Home, and the default Home layout.
#[derive(Debug, Clone)]
pub(crate) struct HomeLayouts {
    browsing: Browsing,
    home_rows: HomeRows,
    /// Holds the default Home layout, which each User's follows but for what they change.
    settings: Settings,
    /// Holds each User's Home layout.
    db: Arc<Database>,
}

impl HomeLayouts {
    /// Composes Home from what `db` holds, through `browsing` and `home_rows`, under the default
    /// Home layout `settings` holds.
    pub(crate) fn new(
        db: Arc<Database>,
        browsing: Browsing,
        home_rows: HomeRows,
        settings: Settings,
    ) -> Self {
        Self {
            browsing,
            home_rows,
            settings,
            db,
        }
    }

    /// The Home rows `user`'s Home shows, in their Home layout's order.
    pub(crate) async fn home_rows(&self, user: &User) -> Result<Vec<HomeRow>, HomeLayoutError> {
        let layout = self.of(user).await?;
        Ok((layout.rows.into_iter())
            .filter_map(|(row, shown)| shown.then_some(row))
            .collect())
    }

    /// `user`'s Home layout, for editing, with every Home row they could see now.
    pub(crate) async fn of(&self, user: &User) -> Result<SeenHomeLayout, HomeLayoutError> {
        let seen = self.seen_rows(user).await?;
        self.arranged(user.id, seen).await
    }

    /// `user`'s Featured, none when their Home layout hides it: what Browse picks from the
    /// Kernel's suggestions and the Plugins' (ADR 0072).
    pub(crate) async fn featured(&self, user: &User) -> Result<Vec<Featured>, HomeLayoutError> {
        let own = UserHomeLayout::of(&self.db, user.id).await?;
        if !own.shows_featured(&self.settings.now().default_home_layout) {
            return Ok(Vec::new());
        }
        let suggested = self.home_rows.featured(user.id).await?;
        let browse = self.browsing.seen_by(user.id).await?;
        Ok(browse.featured(user.id, &suggested).await?)
    }

    /// Saves what `user` asks for in `asked`, over the rows they could see now, and answers their
    /// Home layout as saved. A row `asked` names that they couldn't see changes nothing.
    pub(crate) async fn save(
        &self,
        user: &User,
        asked: AskedHomeLayout,
    ) -> Result<SeenHomeLayout, HomeLayoutError> {
        let seen = self.seen_rows(user).await?;
        let default = self.settings.now().default_home_layout;
        UserHomeLayout::save(&self.db, user.id, asked, ids(&seen), default).await?;
        self.arranged(user.id, seen).await
    }

    /// Saves `asked` as `user`'s order of the rows they could see now, and answers their Home
    /// layout as saved. Of `asked`, only the rows they could see are placed.
    pub(crate) async fn reorder(
        &self,
        user: &User,
        asked: &[HomeRowId],
    ) -> Result<SeenHomeLayout, HomeLayoutError> {
        let seen = self.seen_rows(user).await?;
        let default = self.settings.now().default_home_layout;
        UserHomeLayout::reorder(&self.db, user.id, asked, ids(&seen), default).await?;
        self.arranged(user.id, seen).await
    }

    /// The default Home layout, with every Home row it could place: each Library's, each row a
    /// Plugin shares, and each Plugin's Users' own rows as one entry.
    pub(crate) async fn default_layout(&self) -> Result<SeenHomeLayout, HomeLayoutError> {
        let placeable = self.home_rows.placeable().await?;
        let libraries = self.browsing.everything().libraries().await?;
        let plugin_rows = placeable.into_iter().map(|(id, heading)| {
            // `placeable` gives each shared row with its heading, and the entry for Users' own.
            let shown = if let HomeRowId::Shared { key, .. } = &id {
                ShownRow::Plugin {
                    key: key.clone(),
                    heading,
                    library: None,
                }
            } else {
                ShownRow::UsersOwn
            };
            (id, shown)
        });
        let rows = kernel_rows(&libraries).chain(plugin_rows).collect();
        let default = self.settings.now().default_home_layout;
        Ok(SeenHomeLayout {
            featured_shown: default.featured_shown,
            rows: default.arrange(rows),
        })
    }

    /// Every Home row `user` could see now, in no layout's order: Continue Watching, each Library's
    /// Recently Added row, then the Plugins' rows, each linked to its Library only if `user` sees
    /// it. Lists their Plugin rows as Home does, which records a visit to home.
    async fn seen_rows(&self, user: &User) -> Result<Vec<HomeRow>, HomeLayoutError> {
        let plugin_rows = self.home_rows.list(user).await?;
        let browse = self.browsing.seen_by(user.id).await?;
        let libraries = browse.libraries().await?;
        let sees = |id: &i64| libraries.iter().any(|library| library.id == *id);
        let plugin_rows = plugin_rows.into_iter().map(|(id, row)| {
            let shown = ShownRow::Plugin {
                key: row.key,
                heading: row.heading,
                library: row.library.filter(sees),
            };
            (id, shown)
        });
        let rows = kernel_rows(&libraries).chain(plugin_rows).collect();
        Ok(rows)
    }

    /// `rows`, which the User `user` could see, as their Home layout arranges them.
    async fn arranged(
        &self,
        user: i64,
        rows: Vec<HomeRow>,
    ) -> Result<SeenHomeLayout, HomeLayoutError> {
        let own = UserHomeLayout::of(&self.db, user).await?;
        let default = self.settings.now().default_home_layout;
        Ok(SeenHomeLayout {
            featured_shown: own.shows_featured(&default),
            rows: own.arrange(&default, rows),
        })
    }
}

/// What a Home layout places each of `rows` by.
fn ids(rows: &[HomeRow]) -> Vec<HomeRowId> {
    rows.iter().map(|(id, _)| id.clone()).collect()
}

/// The Kernel's Home rows over `libraries`, in the order they come before the Plugins' rows where
/// no layout places them: Continue Watching, then each Library's Recently Added row.
fn kernel_rows(libraries: &[Library]) -> impl Iterator<Item = HomeRow> {
    let recently_added = libraries.iter().map(|library| {
        let id = HomeRowId::RecentlyAdded {
            library: library.id,
        };
        (id, ShownRow::RecentlyAdded(library.clone()))
    });
    let continue_watching = (HomeRowId::ContinueWatching, ShownRow::ContinueWatching);
    std::iter::once(continue_watching).chain(recently_added)
}

#[cfg(test)]
#[path = "home_layouts_tests.rs"]
mod tests;
