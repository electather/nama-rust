//! Which Quality a Request or a Show is asked, changed and stored at: what Requests does with the
//! rules in `qualities.rs`.

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfiller, Movies, Shows},
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{
    DefaultQuality, Quality, Request, RequestedQuality, request::MovieOrSeason,
};
use sqlx::SqliteConnection;

use super::asked::Asked;
use super::followed::{followed_show_ids, open_followed_show_by_any};
use super::listing::load_one;
use super::state::{ids_json, read_state};
use super::{Error, Requests, qualities};
use crate::{db::now, library::title_source::TitleKind, users::User};

/// A User changing a Request's Quality: a Request manager as they approve or retry it, or an
/// auto-approved Requester whose ask approves it.
#[derive(Debug)]
pub(super) struct NewQuality {
    pub(super) quality: RequestedQuality,
    /// The User id of whoever changes it.
    pub(super) by: i64,
}

impl Requests {
    /// The enabled Plugin providing the Fulfiller of `Kind`, if any.
    pub(super) fn fulfiller<Kind: 'static>(&self) -> Option<PluginId> {
        self.plugins.providers::<dyn Fulfiller<Kind>>().pop_first()
    }

    /// The enabled Plugin providing the Fulfiller for `request`'s kind, Movies or Shows, if any.
    pub(super) fn fulfiller_of(&self, request: &Request) -> Option<PluginId> {
        match request.movie_or_season.as_ref()? {
            MovieOrSeason::Movie(_) => self.fulfiller::<Movies>(),
            MovieOrSeason::Season(_) => self.fulfiller::<Shows>(),
        }
    }

    /// What the ask of `asker`, naming the Quality `chosen` if they did, says about Quality on the
    /// enabled Fulfiller of `Kind`, as [`qualities::for_request`] decides.
    pub(super) async fn quality<Kind: 'static>(
        &self,
        asker: &User,
        chosen: Option<i64>,
    ) -> Result<qualities::Ask, Error> {
        let fulfiller = self.fulfiller::<Kind>();
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::for_request(&mut conn, asker, fulfiller.as_ref(), chosen).await??)
    }

    /// The Qualities of the enabled Fulfiller of `Kind`, in the order an admin listed them, with
    /// `asker`'s personal default among them if [`Self::quality`] would take it. Empty if no
    /// Fulfiller of `Kind` is enabled.
    pub(crate) async fn qualities<Kind: 'static>(
        &self,
        asker: &User,
    ) -> Result<(Vec<Quality>, Option<i64>), sqlx::Error> {
        let Some(fulfiller) = self.fulfiller::<Kind>() else {
            return Ok((Vec::new(), None));
        };
        let mut conn = self.db.reader().acquire().await?;
        qualities::offered_to(&mut conn, asker, &fulfiller).await
    }

    /// `user`'s default Quality for each enabled Fulfiller, as [`qualities::personal_defaults`]
    /// lists them.
    pub(crate) async fn personal_defaults(
        &self,
        user: i64,
    ) -> Result<Vec<DefaultQuality>, sqlx::Error> {
        let movies = self.fulfiller::<Movies>();
        let shows = self.fulfiller::<Shows>();
        let mut conn = self.db.reader().acquire().await?;
        qualities::personal_defaults(&mut conn, user, movies.as_ref(), shows.as_ref()).await
    }

    /// Sets `user`'s default Quality for the Fulfiller from the Plugin `plugin` to `quality`, or
    /// clears it. Refuses a Quality that isn't on its list.
    pub(crate) async fn set_personal_default(
        &self,
        user: i64,
        plugin: PluginId,
        quality: Option<i64>,
    ) -> Result<(), Error> {
        Ok(qualities::set_personal_default(&self.db, user, plugin, quality).await??)
    }

    /// The Qualities the enabled Fulfiller for titles of `kind` offers, none if there is no such
    /// Fulfiller.
    pub(crate) async fn offered_qualities(&self, kind: TitleKind) -> Result<Vec<Quality>, Error> {
        let fulfiller = match kind {
            TitleKind::Movie => self.fulfiller::<Movies>(),
            TitleKind::Show => self.fulfiller::<Shows>(),
        };
        let Some(fulfiller) = fulfiller else {
            return Ok(Vec::new());
        };
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::list(&mut conn, &fulfiller).await?)
    }

    /// `manager` changing the Request `id`'s Quality to the one with the id `quality`, if given,
    /// which must be on the list of the Fulfiller the Request goes to.
    pub(super) async fn new_quality(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Option<NewQuality>, Error> {
        let Some(quality_id) = quality else {
            return Ok(None);
        };
        let request = load_one(&mut *self.db.reader().acquire().await?, id)
            .await?
            .ok_or(Error::NoSuch(Asked::Request, id))?;
        let fulfiller = self.fulfiller_of(&request);
        self.offered_change(manager, fulfiller, quality_id)
            .await
            .map(Some)
    }

    /// `manager` changing a Quality to the one with the id `quality_id`, which must be on the list
    /// of the Fulfiller from the Plugin `fulfiller`.
    pub(super) async fn offered_change(
        &self,
        manager: &User,
        fulfiller: Option<PluginId>,
        quality_id: i64,
    ) -> Result<NewQuality, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let quality = qualities::offered(&mut conn, fulfiller.as_ref(), quality_id).await??;
        Ok(NewQuality {
            quality,
            by: manager.id,
        })
    }
}

/// Changes the Request `id`'s Quality to `new_quality`, if given, as [`requality`] does.
pub(super) async fn change_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    let show = match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => Some(
            season
                .show_ids
                .iter()
                .filter_map(|show| ExternalId::new(&show.namespace, show.value.clone()).ok())
                .collect(),
        ),
        _ => None,
    };
    requality(conn, Some(id), show, new_quality).await
}

/// Changes the Quality of the Followed Show `id`'s Show to `new_quality`, if given, as
/// [`requality`] does.
pub(super) async fn change_followed_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let show = followed_show_ids(conn, id).await?;
    // Every Followed Show stores at least the id it is kept under.
    if show.is_empty() {
        return Err(Error::NoSuch(Asked::FollowedShow, id));
    }
    requality(conn, None, Some(show), new_quality).await
}

/// Changes the Quality of the Request `id`, if given, and of the Show with the ids `show`, if
/// given, to `quality`, where it isn't that one, keeping each change as that Request's latest in
/// place of any earlier one. What the Show's change moves, or why it is refused, is
/// [`qualities::ShowRequests::change`]'s to say.
async fn requality(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    show: Option<ExternalIds>,
    NewQuality { quality, by }: NewQuality,
) -> Result<(), Error> {
    let (seasons, followed) = match &show {
        Some(show) => {
            let moves = show_requests(conn, show).await?.change(&quality)?;
            let followed = if moves.followed {
                open_followed_show_by_any(conn, show).await?
            } else {
                None
            };
            (moves.seasons, followed)
        }
        None => (Vec::new(), None),
    };
    let at = now();
    // ponytail: the Show's other Season Requests change without an event, so a client sees it on
    // its next read; publish one for each if a screen shows them live.
    for request in id.into_iter().chain(seasons) {
        // SQLite reads `quality_name` on the right as it was before this UPDATE.
        sqlx::query!(
            "UPDATE requests
             SET quality_id = ?1, quality_name = ?2, quality_profile_id = ?3,
                 quality_changed_by = ?4, quality_changed_from = quality_name,
                 quality_changed_at = ?5
             WHERE id = ?6 AND quality_id IS NOT ?1",
            quality.id,
            quality.name,
            quality.profile_id,
            by,
            at,
            request
        )
        .execute(&mut *conn)
        .await?;
    }
    if let Some(followed) = followed {
        sqlx::query!(
            "UPDATE followed_shows SET quality_id = ?, quality_name = ?, quality_profile_id = ?
             WHERE id = ?",
            quality.id,
            quality.name,
            quality.profile_id,
            followed
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The Season Requests, in the order they were made, and the open Followed Show of the Show with
/// the ids `show`, each found by any of them, for its Quality rules.
pub(super) async fn show_requests(
    conn: &mut SqliteConnection,
    show: &ExternalIds,
) -> Result<qualities::ShowRequests, sqlx::Error> {
    let show_ids = ids_json(show);
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", state, fulfiller_reference IS NOT NULL AS "handed_off!: bool",
                  quality_id, quality_name, quality_profile_id,
                  quality_used_id, quality_used_name, quality_used_profile_id
           FROM requests
           WHERE season IS NOT NULL AND id IN (
               SELECT title.request_id FROM request_title_ids AS title
               JOIN json_each(?) AS wanted
                   ON title.namespace = wanted.key AND title.value = wanted.value)
           ORDER BY created_at, id"#,
        show_ids
    )
    .fetch_all(&mut *conn)
    .await?;
    let seasons = rows
        .into_iter()
        .map(|row| qualities::SeasonRequest {
            id: row.id,
            asked: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id),
            used: stored_quality(
                row.quality_used_id,
                row.quality_used_name,
                row.quality_used_profile_id,
            ),
            state: read_state(&row.state),
            handed_off: row.handed_off,
        })
        .collect();
    let followed = match open_followed_show_by_any(conn, show).await? {
        Some(id) => sqlx::query!(
            "SELECT quality_id, quality_name, quality_profile_id FROM followed_shows WHERE id = ?",
            id
        )
        .fetch_optional(&mut *conn)
        .await?,
        None => None,
    };
    Ok(qualities::ShowRequests {
        seasons,
        followed: followed.is_some(),
        followed_quality: followed.and_then(|row| {
            stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
        }),
    })
}

/// The three columns a Request stores `quality` in, all set or all `NULL`, as [`stored_quality`]
/// reads them back.
pub(super) fn quality_columns(
    quality: Option<&RequestedQuality>,
) -> (Option<i64>, Option<&str>, Option<&str>) {
    (
        quality.map(|quality| quality.id),
        quality.map(|quality| quality.name.as_str()),
        quality.map(|quality| quality.profile_id.as_str()),
    )
}

/// A Quality a Request stored, if it has one, from the columns [`quality_columns`] gives.
pub(super) fn stored_quality(
    id: Option<i64>,
    name: Option<String>,
    profile_id: Option<String>,
) -> Option<RequestedQuality> {
    Some(RequestedQuality {
        id: id?,
        name: name?,
        profile_id: profile_id?,
        ..Default::default()
    })
}

#[cfg(test)]
#[path = "quality_tests.rs"]
mod tests;
