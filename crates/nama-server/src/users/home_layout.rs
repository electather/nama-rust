//! Each User's Home layout: which Home rows, and whether Featured, they show other than the
//! default Home layout does, and their order once they reorder them (ADR 0077).

use nama_plugin_api::PluginId;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqliteExecutor, Transaction};
use tracing::warn;

use crate::{
    db::Database,
    server_settings::{HomeLayout, HomeRowId, PlacedRow, keep_unnamed},
};

/// What a User shows other than the default Home layout does, and their order once they reorder
/// their rows. Everything else follows the default, so a User who has saved nothing has the
/// default.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UserHomeLayout {
    /// `None` while Featured follows the default.
    featured_shown: Option<bool>,
    /// In no order: [`Self::order`] is.
    rows: Vec<PlacedRow>,
    /// `None` until the User first reorders their rows, so they follow the default's order.
    /// Their own rows from a Plugin are placed one by one, as [`HomeRowId::Own`].
    order: Option<Vec<HomeRowId>>,
}

impl UserHomeLayout {
    /// Whether this User sees Featured under `default`.
    pub(crate) fn shows_featured(&self, default: &HomeLayout) -> bool {
        self.featured_shown.unwrap_or(default.featured_shown)
    }

    /// `rows` in this User's order, or `default`'s until they reorder them, each with whether
    /// this User shows it. Those their order doesn't place go at its end, in `default`'s order.
    pub(crate) fn arrange<T>(
        &self,
        default: &HomeLayout,
        rows: Vec<(HomeRowId, T)>,
    ) -> Vec<((HomeRowId, T), bool)> {
        let mut arranged = default.arrange(rows);
        if let Some(order) = &self.order {
            let place = |row: &HomeRowId| order.iter().position(|placed| placed == row);
            arranged.sort_by_key(|((row, _), _)| place(row).unwrap_or(order.len()));
        }
        for ((row, _), shown) in &mut arranged {
            if let Some(own) = self.rows.iter().find(|own| own.row == *row) {
                *shown = own.shown;
            }
        }
        arranged
    }

    /// This layout once the User asks for `asked` under `default`. A row or Featured they have a
    /// flag for keeps one, now as asked. One they haven't gets a flag only where `asked` differs
    /// from `default`, so what they leave as the default shows it follows the default. The rows
    /// `asked` doesn't name, such as a disabled Plugin's, keep their flags.
    fn asking(self, asked: &HomeLayout, default: &HomeLayout) -> Self {
        let flagged = |row: &HomeRowId| self.rows.iter().any(|own| own.row == *row);
        let named: Vec<PlacedRow> = (asked.rows.iter())
            .filter(|row| flagged(&row.row) || row.shown != default.shows(&row.row))
            .cloned()
            .collect();
        let unnamed = (self.rows.iter())
            .filter(|own| !asked.rows.iter().any(|row| row.row == own.row))
            .cloned();
        let featured_flagged = self.featured_shown.is_some();
        Self {
            featured_shown: (featured_flagged || asked.featured_shown != default.featured_shown)
                .then_some(asked.featured_shown),
            rows: named.into_iter().chain(unnamed).collect(),
            order: self.order,
        }
    }

    /// Every row this layout has a flag or a place for.
    fn placed(self) -> impl Iterator<Item = HomeRowId> {
        let flagged = self.rows.into_iter().map(|placed| placed.row);
        flagged.chain(self.order.into_iter().flatten())
    }

    /// `user`'s Home layout as read through `conn`, the default's if they saved none or it can't
    /// be read.
    async fn stored(conn: impl SqliteExecutor<'_>, user: i64) -> Result<Self, sqlx::Error> {
        let stored = sqlx::query_scalar!(
            "SELECT layout FROM user_home_layouts WHERE user_id = ?",
            user
        )
        .fetch_optional(conn)
        .await?;
        Ok(stored.map_or_default(|json| read(&json, user)))
    }

    /// `user`'s Home layout, the default's if they saved none or it can't be read.
    pub(crate) async fn of(db: &Database, user: i64) -> Result<Self, sqlx::Error> {
        Self::stored(db.reader(), user).await
    }

    /// Saves what `user` asks for in `asked` over `default`. Of `asked`'s rows, only those that
    /// name one of the rows `user` sees, `seen` as [`UserHomeLayout::arrange`] got them, count.
    pub(crate) async fn save(
        db: &Database,
        user: i64,
        asked: &HomeLayout,
        seen: Vec<HomeRowId>,
        default: HomeLayout,
    ) -> Result<(), sqlx::Error> {
        let asked = HomeLayout {
            featured_shown: asked.featured_shown,
            rows: (seen.into_iter())
                .filter_map(|row| {
                    let named = asked.rows.iter().find(|named| named.row.names(&row))?;
                    let shown = named.shown;
                    Some(PlacedRow { row, shown })
                })
                .collect(),
        };
        db.write(move |tx| {
            Box::pin(async move {
                let before = Self::stored(&mut **tx, user).await?;
                store(tx, user, &before.asking(&asked, &default)).await
            })
        })
        .await
    }

    /// Saves `asked` as `user`'s order of the rows they see, `seen` as [`UserHomeLayout::arrange`]
    /// got them, so `default`'s order no longer moves them. Only the rows of `asked` that name
    /// one of `seen` count, each once. The rows their order, or `default`'s on their first reorder,
    /// places that `asked` doesn't, such as a disabled Plugin's, keep their places.
    pub(crate) async fn reorder(
        db: &Database,
        user: i64,
        asked: &[HomeRowId],
        mut seen: Vec<HomeRowId>,
        default: HomeLayout,
    ) -> Result<(), sqlx::Error> {
        let mut order: Vec<HomeRowId> = (asked.iter())
            .filter_map(|named| {
                let at = seen.iter().position(|row| named.names(row))?;
                Some(seen.swap_remove(at))
            })
            .collect();
        let defaults: Vec<HomeRowId> = (default.rows.into_iter())
            .map(|placed| placed.row)
            .collect();
        db.write(move |tx| {
            Box::pin(async move {
                let mut before = Self::stored(&mut **tx, user).await?;
                keep_unnamed(
                    &mut order,
                    &before.order.take().unwrap_or(defaults),
                    PartialEq::eq,
                    // The default's entry for a Plugin's Users' own rows places none of them one
                    // by one, so it isn't copied: a User's own row their order lacks goes at its end.
                    |row| !matches!(row, HomeRowId::UsersOwn { .. }),
                );
                let order = Some(order);
                store(tx, user, &Self { order, ..before }).await
            })
        })
        .await
    }

    /// Forgets `user`'s Home layout, so they follow the default.
    pub(crate) async fn reset(db: &Database, user: i64) -> Result<(), sqlx::Error> {
        db.write(move |tx| {
            Box::pin(async move {
                sqlx::query!("DELETE FROM user_home_layouts WHERE user_id = ?", user)
                    .execute(&mut **tx)
                    .await
                    .map(drop)
            })
        })
        .await
    }

    /// Drops every User's flags and order for the rows of each Plugin `has` says the Server no
    /// longer has.
    // ponytail: reads every User's layout, a handful of rows each; fine for a household Server.
    pub(crate) async fn forget_uninstalled(
        db: &Database,
        has: impl Fn(&PluginId) -> bool,
    ) -> Result<(), sqlx::Error> {
        let stored = sqlx::query!("SELECT user_id, layout FROM user_home_layouts")
            .fetch_all(db.reader())
            .await?;
        let gone: Vec<PluginId> = (stored.iter())
            .flat_map(|row| read(&row.layout, row.user_id).placed())
            .filter_map(|placed| placed.plugin().cloned())
            .filter(|plugin| !has(plugin))
            .collect();
        if gone.is_empty() {
            return Ok(());
        }
        // Read again in the write, so a layout saved meanwhile isn't lost.
        db.write(move |tx| Box::pin(async move { forget(tx, &gone).await }))
            .await
    }
}

/// Drops every User's flags and order for the rows of the Plugins `gone` names.
async fn forget(tx: &mut Transaction<'_, Sqlite>, gone: &[PluginId]) -> Result<(), sqlx::Error> {
    let layouts = sqlx::query!("SELECT user_id, layout FROM user_home_layouts")
        .fetch_all(&mut **tx)
        .await?;
    for row in layouts {
        let mut layout = read(&row.layout, row.user_id);
        let kept = |placed: &HomeRowId| placed.plugin().is_none_or(|it| !gone.contains(it));
        layout.rows.retain(|placed| kept(&placed.row));
        if let Some(order) = &mut layout.order {
            order.retain(kept);
        }
        store(tx, row.user_id, &layout).await?;
    }
    Ok(())
}

/// The layout stored as `json` for `user`, or the default's, logged, if it can't be read.
fn read(json: &str, user: i64) -> UserHomeLayout {
    let read = serde_json::from_str(json);
    read.inspect_err(|err| warn!(user, "a User's Home layout reads as none: {err}"))
        .unwrap_or_default()
}

async fn store(
    tx: &mut Transaction<'_, Sqlite>,
    user: i64,
    layout: &UserHomeLayout,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(layout).map_err(|err| sqlx::Error::Encode(err.into()))?;
    sqlx::query!(
        "INSERT INTO user_home_layouts (user_id, layout) VALUES (?, ?)
         ON CONFLICT (user_id) DO UPDATE SET layout = excluded.layout",
        user,
        json
    )
    .execute(&mut **tx)
    .await
    .map(drop)
}
