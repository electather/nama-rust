//! Qualities (ADR 0025): the service quality profiles an admin offers on each Fulfiller's list,
//! under friendly names, with one the default a new Request takes, and every rule about which
//! Quality a Request, Show or Followed Show takes.
//!
//! It owns them and their tables (see "One owner for Quality rules" in `src/requests/AGENTS.md`).

use std::collections::HashSet;

use nama_plugin_api::{
    PluginId,
    fulfiller::Fulfilled,
    settings::{Choice, Choices},
};
use nama_proto::nama::v1::{
    DefaultQuality, ExtensionPoint, Permission, Quality, RequestState, RequestedQuality,
};
use sqlx::SqliteConnection;

use super::state::is_open;
use crate::{db::Database, users::User};

/// The choice setting whose choices are the profiles a Fulfiller's Qualities may offer. A
/// Fulfiller takes a Request's profile in its place, so the setting stays the fallback.
pub(crate) const QUALITY_PROFILE: &str = "quality_profile";

/// The parts of a Quality a list can leave blank or repeat, by their proto field names.
pub(crate) const PROFILE: &str = "profile_id";
pub(crate) const NAME: &str = "name";

/// The proto field an asker names a Quality in as they ask.
pub(crate) const ASKED: &str = "quality";
/// The proto field a Request manager names a Quality in as they approve or retry.
pub(crate) const CHANGED: &str = "quality_id";

/// Why a list of Qualities can't be saved. The `usize` is the index of the Quality at fault.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Invalid {
    #[error("a Quality has no {1}")]
    Blank(usize, &'static str),
    #[error("two Qualities have the same {1}")]
    Repeated(usize, &'static str),
    #[error("exactly one Quality must be the default")]
    NotOneDefault,
    #[error("the Fulfiller doesn't offer the quality profile {1}")]
    ProfileNotOffered(usize, String),
}

/// Why a User can't have the Quality they named.
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum Refused {
    #[error("choosing a Quality needs PERMISSION_CHOOSE_QUALITY")]
    NotAllowed,
    /// `field` is where they named it, [`ASKED`] or [`CHANGED`].
    #[error("the Fulfiller doesn't offer the Quality {id}")]
    NotOffered { id: i64, field: &'static str },
    /// An asker named a Quality other than the Show's, which each of its Seasons takes.
    #[error("each Season of the Show takes its Quality {}, not the Quality {named}", .shows.name)]
    NotTheShows { named: i64, shows: RequestedQuality },
    /// A Request manager changed the Show's Quality once a Season was handed off or Approved,
    /// after which Sonarr keeps the profile it added the series with.
    #[error("the Show's Quality {} is fixed, as a Season of it was approved", .shows.name)]
    ShowQualityFixed { named: i64, shows: RequestedQuality },
}

/// Checks `qualities` against the list rules: each has a profile and a name, no profile or name
/// (ignoring case) is given twice, each profile is one of the `offered` choices for
/// [`QUALITY_PROFILE`], and a list with any has exactly one default.
pub(crate) fn check(qualities: &[Quality], offered: &Choices) -> Result<(), Invalid> {
    let (mut profiles, mut names) = (HashSet::new(), HashSet::new());
    for (index, quality) in qualities.iter().enumerate() {
        let parts = [
            (PROFILE, quality.profile_id.clone(), &mut profiles),
            (NAME, quality.name.trim().to_lowercase(), &mut names),
        ];
        for (part, value, seen) in parts {
            if value.is_empty() {
                return Err(Invalid::Blank(index, part));
            }
            if !seen.insert(value) {
                return Err(Invalid::Repeated(index, part));
            }
        }
    }
    let defaults = qualities
        .iter()
        .filter(|quality| quality.is_default)
        .count();
    if !qualities.is_empty() && defaults != 1 {
        return Err(Invalid::NotOneDefault);
    }
    let offered = offered.get(QUALITY_PROFILE).map_or(&[][..], Vec::as_slice);
    let not_offered = qualities.iter().enumerate().find(|(_, quality)| {
        !offered
            .iter()
            .any(|Choice { value, .. }| *value == quality.profile_id)
    });
    not_offered.map_or(Ok(()), |(index, quality)| {
        Err(Invalid::ProfileNotOffered(
            index,
            quality.profile_id.clone(),
        ))
    })
}

/// The Plugin `plugin`'s Qualities, in the order the admin listed them.
pub(crate) async fn list(
    conn: &mut SqliteConnection,
    plugin: &PluginId,
) -> Result<Vec<Quality>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", profile_id, name, description, is_default AS "is_default: bool"
           FROM qualities WHERE plugin_id = ? ORDER BY position"#,
        plugin
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Quality {
            id: row.id,
            profile_id: row.profile_id,
            name: row.name,
            description: row.description,
            is_default: row.is_default,
            ..Default::default()
        })
        .collect())
}

/// Replaces the Plugin `plugin`'s Qualities with `qualities`, which [`check`] passed, and returns
/// them as saved. A Quality keeps its id if `plugin`'s list has it and no earlier Quality in
/// `qualities` kept it; any other gets a new one.
pub(crate) async fn replace(
    db: &Database,
    plugin: PluginId,
    qualities: Vec<Quality>,
) -> Result<Vec<Quality>, sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            let id = plugin.as_str();
            let mut listed: HashSet<i64> = list(tx, &plugin)
                .await?
                .into_iter()
                .map(|quality| quality.id)
                .collect();
            // Deleted first, so a name or profile moving between Qualities can't trip the UNIQUE
            // constraints halfway.
            sqlx::query!("DELETE FROM qualities WHERE plugin_id = ?", id)
                .execute(&mut **tx)
                .await?;
            for (position, quality) in (0_i64..).zip(&qualities) {
                let kept_id = listed.remove(&quality.id).then_some(quality.id);
                let name = quality.name.trim();
                let description = quality.description.trim();
                sqlx::query!(
                    "INSERT INTO qualities
                         (id, plugin_id, position, profile_id, name, description, is_default)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    kept_id,
                    id,
                    position,
                    quality.profile_id,
                    name,
                    description,
                    quality.is_default
                )
                .execute(&mut **tx)
                .await?;
            }
            list(tx, &plugin).await
        })
    })
    .await
}

/// Deletes the Fulfiller `plugin`'s Qualities and every User's default Quality for it.
pub(crate) async fn delete_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM qualities WHERE plugin_id = ?", plugin)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM default_qualities WHERE plugin_id = ?", plugin)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// What an ask says about Quality, which [`for_request`] and [`ShowRequests`] make and Requests
/// carries to the Requests it opens or joins.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Ask {
    /// The Quality a new Request for it takes, if its Fulfiller has any.
    taken: Option<RequestedQuality>,
    /// Whether the asker chose `taken`, by picking it or as their personal default, so it replaces
    /// a Pending Request's when their ask approves it.
    chosen: bool,
    /// The id of the Quality the asker named, if they did, which must be the Show's if it has one.
    named: Option<i64>,
}

impl Ask {
    /// The Quality a new Request for it takes, if its Fulfiller has any.
    pub(crate) fn taken(&self) -> Option<&RequestedQuality> {
        self.taken.as_ref()
    }

    /// The Quality an auto-approved joiner's ask approves a Pending Request at: the one taken if
    /// they chose it, by picking it or as a personal default still on the list, else `None`, so
    /// the Request keeps its own and its earlier Requesters aren't moved to the default.
    pub(crate) fn joiner_approves_at(&self) -> Option<RequestedQuality> {
        self.taken.clone().filter(|_| self.chosen)
    }
}

/// What the ask of `asker`, naming the Quality `named` if they did, says about Quality on the list
/// of the Fulfiller from the Plugin `fulfiller`. A new Request takes the Quality they named, else
/// their personal default if they may choose and the list still has it, both as chosen, else the
/// list's default. None without a list.
///
/// Refuses a `named` Quality from an `asker` without `CHOOSE_QUALITY`, or one not on the list.
pub(crate) async fn for_request(
    conn: &mut SqliteConnection,
    asker: &User,
    fulfiller: Option<&PluginId>,
    named: Option<i64>,
) -> Result<Result<Ask, Refused>, sqlx::Error> {
    if named.is_some() && !may_choose(asker) {
        return Ok(Err(Refused::NotAllowed));
    }
    let (listed, personal_default) = match fulfiller {
        Some(fulfiller) => offered_to(conn, asker, fulfiller).await?,
        None => (Vec::new(), None),
    };
    Ok(pick(&listed, named, personal_default))
}

/// [`for_request`]'s choice on `listed`, where `personal_default` is one the asker may use.
fn pick(
    listed: &[Quality],
    named: Option<i64>,
    personal_default: Option<i64>,
) -> Result<Ask, Refused> {
    let find = |id| listed.iter().find(|quality| quality.id == id);
    let as_chosen = |quality| Ask {
        taken: Some(requested(quality)),
        chosen: true,
        named,
    };
    if let Some(id) = named {
        return find(id)
            .map(as_chosen)
            .ok_or(Refused::NotOffered { id, field: ASKED });
    }
    if let Some(quality) = personal_default.and_then(find) {
        return Ok(as_chosen(quality));
    }
    let default = listed.iter().find(|quality| quality.is_default);
    Ok(Ask {
        taken: default.map(requested),
        ..Ask::default()
    })
}

/// The Plugin `fulfiller`'s Qualities, in the order an admin listed them, with `asker`'s personal
/// default among them if [`for_request`] would take it.
pub(crate) async fn offered_to(
    conn: &mut SqliteConnection,
    asker: &User,
    fulfiller: &PluginId,
) -> Result<(Vec<Quality>, Option<i64>), sqlx::Error> {
    let listed = list(conn, fulfiller).await?;
    let personal_default = if may_choose(asker) {
        personal_default(conn, asker.id, fulfiller)
            .await?
            .filter(|id| listed.iter().any(|quality| quality.id == *id))
    } else {
        None
    };
    Ok((listed, personal_default))
}

/// `user`'s default Quality for the Movies Fulfiller from the Plugin `movies` and the Shows one
/// from `shows`, in that order, for each whose list has more than one, or has one left after an
/// admin removed `user`'s, to flag it.
pub(crate) async fn personal_defaults(
    conn: &mut SqliteConnection,
    user: i64,
    movies: Option<&PluginId>,
    shows: Option<&PluginId>,
) -> Result<Vec<DefaultQuality>, sqlx::Error> {
    let fulfillers = [
        (movies, ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER),
        (shows, ExtensionPoint::EXTENSION_POINT_SHOW_FULFILLER),
    ];
    let mut defaults = Vec::new();
    for (plugin, fulfiller) in fulfillers {
        let Some(plugin) = plugin else {
            continue;
        };
        let qualities = list(conn, plugin).await?;
        let quality = personal_default(conn, user, plugin).await?;
        if shows_default(&qualities, quality) {
            defaults.push(DefaultQuality {
                quality,
                plugin_id: plugin.to_string(),
                fulfiller: fulfiller.into(),
                qualities,
                ..Default::default()
            });
        }
    }
    Ok(defaults)
}

/// Whether a User's settings show their default for a Fulfiller whose list is `listed`: to pick
/// one when it has more than one, or to flag `theirs` once an admin removed it from a list that
/// still has one.
fn shows_default(listed: &[Quality], theirs: Option<i64>) -> bool {
    let removed = theirs.is_some_and(|id| listed.iter().all(|quality| quality.id != id));
    listed.len() > 1 || (removed && !listed.is_empty())
}

/// Whether `user` may choose a Quality, so naming one or their personal default counts.
fn may_choose(user: &User) -> bool {
    user.permissions.allow(Permission::ChooseQuality)
}

/// A copy of the Quality with the id `id` on the list of the Fulfiller from the Plugin
/// `fulfiller`, for a Request manager to change a Request or Show to. Refuses one the list hasn't.
pub(crate) async fn offered(
    conn: &mut SqliteConnection,
    fulfiller: Option<&PluginId>,
    id: i64,
) -> Result<Result<RequestedQuality, Refused>, sqlx::Error> {
    let listed = match fulfiller {
        Some(fulfiller) => list(conn, fulfiller).await?,
        None => Vec::new(),
    };
    let quality = listed.iter().find(|quality| quality.id == id);
    Ok(quality
        .map(requested)
        .ok_or(Refused::NotOffered { id, field: CHANGED }))
}

/// The copy of `quality` a Request keeps, so the Quality's renaming or removal doesn't change it.
fn requested(quality: &Quality) -> RequestedQuality {
    RequestedQuality {
        id: quality.id,
        name: quality.name.clone(),
        profile_id: quality.profile_id.clone(),
        ..Default::default()
    }
}

/// The id of the Quality `user` set as their default for the Plugin `plugin`, which its list may
/// no longer have.
async fn personal_default(
    conn: &mut SqliteConnection,
    user: i64,
    plugin: &PluginId,
) -> Result<Option<i64>, sqlx::Error> {
    let plugin = plugin.as_str();
    sqlx::query_scalar!(
        "SELECT quality_id FROM default_qualities WHERE user_id = ? AND plugin_id = ?",
        user,
        plugin
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Sets `user`'s default Quality for the Plugin `plugin` to `quality`, or clears it for `None`.
/// Refuses, changing nothing, a Quality `plugin`'s list hasn't.
pub(crate) async fn set_personal_default(
    db: &Database,
    user: i64,
    plugin: PluginId,
    quality: Option<i64>,
) -> Result<Result<(), Refused>, sqlx::Error> {
    let stored = db
        .write::<_, sqlx::Error, _>(move |tx| {
            Box::pin(async move {
                let plugin = plugin.as_str();
                let Some(quality) = quality else {
                    sqlx::query!(
                        "DELETE FROM default_qualities WHERE user_id = ? AND plugin_id = ?",
                        user,
                        plugin
                    )
                    .execute(&mut **tx)
                    .await?;
                    return Ok(true);
                };
                let stored = sqlx::query!(
                    "INSERT INTO default_qualities (user_id, plugin_id, quality_id)
                 SELECT ?1, ?2, id FROM qualities WHERE plugin_id = ?2 AND id = ?3
                 ON CONFLICT (user_id, plugin_id) DO UPDATE SET quality_id = excluded.quality_id",
                    user,
                    plugin,
                    quality
                )
                .execute(&mut **tx)
                .await?;
                Ok(stored.rows_affected() == 1)
            })
        })
        .await?;
    Ok(match quality {
        Some(id) if !stored => Err(Refused::NotOffered { id, field: ASKED }),
        _ => Ok(()),
    })
}

/// A Show's Season Requests and open Followed Show as its Quality rules see them, which Requests
/// loads in the write that acts on what they answer, so two writes can't give it two Qualities.
#[derive(Debug, Default)]
pub(crate) struct ShowRequests {
    /// Its Season Requests, in the order they were made.
    pub(crate) seasons: Vec<SeasonRequest>,
    /// Whether it has an open Followed Show.
    pub(crate) followed: bool,
    /// That Followed Show's Quality, if it has one.
    pub(crate) followed_quality: Option<RequestedQuality>,
}

/// A Season Request as its Show's Quality rules see it.
#[derive(Debug)]
pub(crate) struct SeasonRequest {
    pub(crate) id: i64,
    /// The Quality it was asked at, if it took one.
    pub(crate) asked: Option<RequestedQuality>,
    /// The Quality its hand-off reported the service used, if it said.
    pub(crate) used: Option<RequestedQuality>,
    pub(crate) state: RequestState,
    /// Whether its Fulfiller took it.
    pub(crate) handed_off: bool,
}

/// What a Request manager's change of a Show's Quality moves.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Moves {
    /// The ids of its open Season Requests not yet handed off. A handed-off one keeps what it
    /// was asked at.
    pub(crate) seasons: Vec<i64>,
    /// Whether its open Followed Show moves, which it does whenever there is one.
    pub(crate) followed: bool,
}

impl SeasonRequest {
    fn is_open(&self) -> bool {
        is_open(self.state)
    }

    /// The Quality it holds its Show to: what the service used, as Sonarr keeps a series'
    /// profile, else what it was asked at.
    fn quality(&self) -> Option<&RequestedQuality> {
        self.used.as_ref().or(self.asked.as_ref())
    }
}

impl ShowRequests {
    /// The Show's Quality, which each of its Season Requests takes: that of its earliest Season
    /// Request with one that was handed off, else that of its earliest open one, else its open
    /// Followed Show's.
    pub(crate) fn quality(&self) -> Option<&RequestedQuality> {
        let asked = || self.seasons.iter().filter(|season| season.asked.is_some());
        let season = asked()
            .find(|season| season.handed_off)
            .or_else(|| asked().find(|season| season.is_open()));
        season
            .and_then(SeasonRequest::quality)
            .or(self.followed_quality.as_ref())
    }

    /// Whether the Show's Quality is fixed: once a Season with a Quality was handed off, or is
    /// Approved, as its hand-off may be under way.
    pub(crate) fn is_fixed(&self) -> bool {
        self.seasons.iter().any(|season| {
            season.asked.is_some() && (season.handed_off || season.state == RequestState::Approved)
        })
    }

    /// What `asked`, as [`for_request`] made it, says about Quality for new Season Requests of
    /// the Show: as [`Self::for_scan`] says if the Show has a Quality, else `asked`. Refuses
    /// another Quality the asker named.
    pub(crate) fn for_seasons(&self, asked: Ask) -> Result<Ask, Refused> {
        let Some(shows) = self.quality() else {
            return Ok(asked);
        };
        if let Some(named) = asked.named.filter(|&id| id != shows.id) {
            let shows = shows.clone();
            return Err(Refused::NotTheShows { named, shows });
        }
        Ok(self.for_scan())
    }

    /// What a scan's ask for a new Season of the Show says about Quality: it takes the Show's,
    /// which nobody chose.
    pub(crate) fn for_scan(&self) -> Ask {
        Ask {
            taken: self.quality().cloned(),
            ..Ask::default()
        }
    }

    /// What a Request manager changing the Show's Quality to `new` moves, so the Show keeps one
    /// Quality. Refuses another Quality once the Show's is fixed.
    pub(crate) fn change(&self, new: &RequestedQuality) -> Result<Moves, Refused> {
        if self.is_fixed()
            && let Some(shows) = self.quality()
            && shows.id != new.id
        {
            let shows = shows.clone();
            return Err(Refused::ShowQualityFixed {
                named: new.id,
                shows,
            });
        }
        let seasons = self
            .seasons
            .iter()
            .filter(|season| season.is_open() && !season.handed_off)
            .map(|season| season.id)
            .collect();
        Ok(Moves {
            seasons,
            followed: self.followed,
        })
    }
}

/// The Quality the service downloads a Request asked at `asked` at, as its hand-off reported in
/// `fulfilled`: `asked` if it has that profile, else the Quality on its Fulfiller's list, `listed`,
/// with that profile, else the service's own profile with no Quality id. `None` if the Request took
/// no Quality, so the Plugin's own setting applied, or the Fulfiller didn't say.
pub(crate) fn used(
    asked: Option<&RequestedQuality>,
    listed: &[Quality],
    fulfilled: &Fulfilled,
) -> Option<RequestedQuality> {
    let Fulfilled {
        profile_id,
        profile_name,
        ..
    } = fulfilled;
    let asked = asked?;
    if profile_id.is_empty() {
        return None;
    }
    if asked.profile_id == *profile_id {
        return Some(asked.clone());
    }
    let on_the_list = listed
        .iter()
        .find(|quality| quality.profile_id == *profile_id);
    // ponytail: id 0 means "not on the list"; a domain type with an optional id is a later
    // refactor.
    Some(on_the_list.map_or_else(
        || RequestedQuality {
            name: profile_name.clone(),
            profile_id: profile_id.clone(),
            ..Default::default()
        },
        requested,
    ))
}

/// Lists one Quality for the Fulfiller `plugin` and sets it as `user`'s default, for a test of
/// what a removed Plugin leaves behind.
#[cfg(test)]
pub(crate) async fn leave_qualities(db: &Database, plugin: &PluginId, user: i64) {
    let hd = Quality {
        profile_id: "4".to_owned(),
        name: "HD".to_owned(),
        is_default: true,
        ..Default::default()
    };
    let saved = replace(db, plugin.clone(), vec![hd]).await.unwrap();
    let default = Some(saved[0].id);
    (set_personal_default(db, user, plugin.clone(), default).await)
        .unwrap()
        .unwrap();
}

/// Whether the Fulfiller `plugin` still lists Qualities, and `user` a default Quality for it.
#[cfg(test)]
pub(crate) async fn qualities_left(db: &Database, plugin: &PluginId, user: i64) -> [bool; 2] {
    let mut conn = db.reader().acquire().await.unwrap();
    let listed = list(&mut conn, plugin).await.unwrap();
    let default = personal_default(&mut conn, user, plugin).await.unwrap();
    [!listed.is_empty(), default.is_some()]
}

#[cfg(test)]
#[path = "qualities_tests.rs"]
mod tests;
