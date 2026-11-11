//! Qualities (ADR 0025): the service quality profiles an admin offers on each Fulfiller's list,
//! under friendly names, with one the default a new Request takes, and every rule about which
//! Quality a Request, Show or Followed Show takes.
//!
//! It owns them and their tables (see the crate's `AGENTS.md`, "One owner for Quality rules").

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

use crate::{db::Database, requests, users::User};

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
        requests::is_open(self.state)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use buffa::EnumValue;
    use tempfile::TempDir;

    use super::*;

    fn quality(profile_id: &str, name: &str, is_default: bool) -> Quality {
        Quality {
            profile_id: profile_id.to_owned(),
            name: name.to_owned(),
            is_default,
            ..Default::default()
        }
    }

    /// The choices a settings check offers for [`QUALITY_PROFILE`]: the profiles `ids`.
    fn profiles(ids: &[&str]) -> Choices {
        let choices = ids.iter().map(|&id| Choice {
            value: id.to_owned(),
            label: id.to_owned(),
        });
        Choices::from([(QUALITY_PROFILE.to_owned(), choices.collect())])
    }

    #[test]
    fn an_empty_list_or_one_with_one_default_and_nothing_repeated_passes() {
        let offered = [quality("4", "HD", true), quality("5", "4K", false)];

        assert_eq!(check(&[], &Choices::new()), Ok(()));
        assert_eq!(check(&offered, &profiles(&["4", "5", "6"])), Ok(()));
    }

    #[test]
    fn a_blank_name_or_profile_is_named_by_index() {
        let blank_name = [quality("4", "HD", true), quality("5", "  ", false)];
        let blank_profile = [quality("", "HD", true)];
        let offered = profiles(&["4", "5"]);

        assert_eq!(check(&blank_name, &offered), Err(Invalid::Blank(1, NAME)));
        assert_eq!(
            check(&blank_profile, &offered),
            Err(Invalid::Blank(0, PROFILE))
        );
    }

    #[test]
    fn a_profile_given_twice_or_a_name_in_another_case_or_with_spaces_is_repeated() {
        let profiles_twice = [quality("4", "HD", true), quality("4", "Full HD", false)];
        let names = [quality("4", "HD", true), quality("5", " hd ", false)];
        let offered = profiles(&["4", "5"]);

        assert_eq!(
            check(&profiles_twice, &offered),
            Err(Invalid::Repeated(1, PROFILE))
        );
        assert_eq!(check(&names, &offered), Err(Invalid::Repeated(1, NAME)));
    }

    #[test]
    fn a_list_needs_exactly_one_default() {
        let none = [quality("4", "HD", false)];
        let two = [quality("4", "HD", true), quality("5", "4K", true)];
        let offered = profiles(&["4", "5"]);

        assert_eq!(check(&none, &offered), Err(Invalid::NotOneDefault));
        assert_eq!(check(&two, &offered), Err(Invalid::NotOneDefault));
    }

    #[test]
    fn a_profile_the_fulfiller_doesnt_offer_is_named_by_index() {
        let listed = [quality("4", "HD", true), quality("5", "4K", false)];

        assert_eq!(
            check(&listed, &profiles(&["4"])),
            Err(Invalid::ProfileNotOffered(1, "5".to_owned()))
        );
        assert_eq!(
            check(&listed, &Choices::new()),
            Err(Invalid::ProfileNotOffered(0, "4".to_owned()))
        );
    }

    const RADARR: &str = "radarr";

    /// A database whose Fulfiller [`RADARR`] offers HD, the default, and 4K, with a User who set
    /// 4K as their default.
    struct Listed {
        db: Arc<Database>,
        plugin: PluginId,
        hd: i64,
        four_k: i64,
        user: i64,
        _data: TempDir,
    }

    impl Listed {
        async fn new() -> Self {
            let data = tempfile::tempdir().unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let plugin = PluginId::from(RADARR.to_owned());
            let listed = [quality("4", "HD", true), quality("5", "4K", false)];
            let saved = replace(&db, plugin.clone(), listed.into()).await.unwrap();
            let user = db
                .write(|tx| {
                    Box::pin(
                        sqlx::query_scalar(
                            "INSERT INTO users (username, display_username, permissions,
                                                created_at, updated_at)
                             VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
                        )
                        .fetch_one(&mut **tx),
                    )
                })
                .await
                .unwrap();
            let four_k = saved[1].id;
            set_personal_default(&db, user, plugin.clone(), Some(four_k))
                .await
                .unwrap()
                .unwrap();
            Self {
                db,
                plugin,
                hd: saved[0].id,
                four_k,
                user,
                _data: data,
            }
        }

        /// The User, holding `permissions`.
        fn asker(&self, permissions: &[Permission]) -> User {
            User {
                id: self.user,
                name: "Ada".to_owned(),
                permissions: permissions.iter().copied().collect(),
                created_at: 0,
            }
        }

        /// What [`for_request`] answers `asker` naming `named`, on [`RADARR`]'s list if `listed`,
        /// else with no Fulfiller, by the Quality's name.
        async fn ask(
            &self,
            asker: &User,
            listed: bool,
            named: Option<i64>,
        ) -> Result<(Option<String>, bool), Refused> {
            let mut conn = self.db.reader().acquire().await.unwrap();
            let fulfiller = listed.then_some(&self.plugin);
            let ask = for_request(&mut conn, asker, fulfiller, named)
                .await
                .unwrap()?;
            Ok((ask.taken.map(|quality| quality.name), ask.chosen))
        }

        /// What [`personal_defaults`] lists for the User with the Movies Fulfiller from `movies`
        /// and the Shows one from `shows`, by Fulfiller and the User's default.
        async fn defaults(
            &self,
            movies: Option<&PluginId>,
            shows: Option<&PluginId>,
        ) -> Vec<(EnumValue<ExtensionPoint>, Option<i64>)> {
            let mut conn = self.db.reader().acquire().await.unwrap();
            personal_defaults(&mut conn, self.user, movies, shows)
                .await
                .unwrap()
                .into_iter()
                .map(|default| (default.fulfiller, default.quality))
                .collect()
        }
    }

    fn taken(name: &str, chosen: bool) -> (Option<String>, bool) {
        (Some(name.to_owned()), chosen)
    }

    #[tokio::test]
    async fn a_chooser_gets_the_quality_they_name_else_their_default_both_as_chosen() {
        let listed = Listed::new().await;
        let chooser = listed.asker(&[Permission::ChooseQuality]);

        assert_eq!(
            listed.ask(&chooser, true, Some(listed.hd)).await,
            Ok(taken("HD", true))
        );
        assert_eq!(
            listed.ask(&chooser, true, None).await,
            Ok(taken("4K", true))
        );
    }

    #[tokio::test]
    async fn a_default_no_longer_on_the_list_gives_way_to_the_lists_default() {
        let listed = Listed::new().await;
        let chooser = listed.asker(&[Permission::ChooseQuality]);
        let hd_only = vec![Quality {
            id: listed.hd,
            ..quality("4", "HD", true)
        }];
        replace(&listed.db, listed.plugin.clone(), hd_only)
            .await
            .unwrap();

        assert_eq!(
            listed.ask(&chooser, true, None).await,
            Ok(taken("HD", false))
        );
    }

    #[tokio::test]
    async fn without_choose_quality_naming_one_is_refused_and_a_default_is_ignored() {
        let listed = Listed::new().await;
        let member = listed.asker(&[Permission::Request]);

        assert_eq!(
            listed.ask(&member, true, Some(listed.hd)).await,
            Err(Refused::NotAllowed)
        );
        assert_eq!(
            listed.ask(&member, true, None).await,
            Ok(taken("HD", false))
        );
        let (_, usable) = offered_to(
            &mut listed.db.reader().acquire().await.unwrap(),
            &member,
            &listed.plugin,
        )
        .await
        .unwrap();
        assert_eq!(usable, None);
    }

    #[tokio::test]
    async fn a_quality_not_on_the_list_or_with_no_fulfiller_is_refused_naming_it() {
        let listed = Listed::new().await;
        let chooser = listed.asker(&[Permission::ChooseQuality]);
        let not_offered = listed.four_k + 1;

        let off_the_list = listed.ask(&chooser, true, Some(not_offered)).await;
        let no_fulfiller = listed.ask(&chooser, false, Some(listed.hd)).await;

        let refused = |id| Err(Refused::NotOffered { id, field: ASKED });
        assert_eq!(off_the_list, refused(not_offered));
        assert_eq!(no_fulfiller, refused(listed.hd));
        assert_eq!(listed.ask(&chooser, false, None).await, Ok((None, false)));
    }

    #[tokio::test]
    async fn a_request_managers_quality_must_be_on_the_list() {
        let listed = Listed::new().await;
        let mut conn = listed.db.reader().acquire().await.unwrap();

        let on_it = offered(&mut conn, Some(&listed.plugin), listed.four_k).await;
        let off_it = offered(&mut conn, Some(&listed.plugin), 999).await;

        assert_eq!(
            on_it.unwrap().map(|quality| quality.name),
            Ok("4K".to_owned())
        );
        assert_eq!(
            off_it.unwrap(),
            Err(Refused::NotOffered {
                id: 999,
                field: CHANGED
            })
        );
    }

    #[tokio::test]
    async fn a_personal_default_off_the_list_is_refused_and_a_cleared_one_is_unset() {
        let listed = Listed::new().await;
        let set =
            |quality| set_personal_default(&listed.db, listed.user, listed.plugin.clone(), quality);

        let refused = set(Some(999)).await;
        set(None).await.unwrap().unwrap();

        assert_eq!(
            refused.unwrap(),
            Err(Refused::NotOffered {
                id: 999,
                field: ASKED
            })
        );
        let mut conn = listed.db.reader().acquire().await.unwrap();
        let cleared = personal_default(&mut conn, listed.user, &listed.plugin).await;
        assert_eq!(cleared.unwrap(), None);
    }

    #[tokio::test]
    async fn a_quality_id_given_twice_is_kept_once_and_the_other_quality_gets_a_new_one() {
        let listed = Listed::new().await;
        let with_hds_id = |name, is_default| Quality {
            id: listed.hd,
            ..quality(name, name, is_default)
        };

        let saved = replace(
            &listed.db,
            listed.plugin.clone(),
            vec![with_hds_id("HD", true), with_hds_id("8K", false)],
        )
        .await
        .unwrap();

        assert_eq!(saved[0].id, listed.hd);
        assert!(![listed.hd, listed.four_k].contains(&saved[1].id));
    }

    #[test]
    fn settings_show_a_default_to_pick_from_two_or_to_flag_one_an_admin_removed() {
        let hd = Quality {
            id: 1,
            ..quality("4", "HD", true)
        };
        let four_k = Quality {
            id: 2,
            ..quality("5", "4K", false)
        };
        let both = [hd, four_k];
        let hd_only = &both[..1];

        assert!(shows_default(&both, None));
        assert!(shows_default(hd_only, Some(2)));
        assert!(!shows_default(hd_only, Some(1)));
        assert!(!shows_default(&[], Some(2)));
    }

    #[tokio::test]
    async fn a_users_defaults_list_each_fulfiller_with_several_qualities_or_with_theirs_removed() {
        let listed = Listed::new().await;
        let plugin = Some(&listed.plugin);
        let movies = ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER.into();
        let shows = ExtensionPoint::EXTENSION_POINT_SHOW_FULFILLER.into();
        let four_k = Some(listed.four_k);

        let several = listed.defaults(plugin, plugin).await;
        let no_fulfiller = listed.defaults(None, None).await;
        let hd_only = vec![Quality {
            id: listed.hd,
            ..quality("4", "HD", true)
        }];
        replace(&listed.db, listed.plugin.clone(), hd_only)
            .await
            .unwrap();
        let removed = listed.defaults(None, plugin).await;
        set_personal_default(
            &listed.db,
            listed.user,
            listed.plugin.clone(),
            Some(listed.hd),
        )
        .await
        .unwrap()
        .unwrap();
        let one = listed.defaults(plugin, None).await;

        assert_eq!(several, [(movies, four_k), (shows, four_k)]);
        assert_eq!(no_fulfiller, []);
        assert_eq!(removed, [(shows, four_k)]);
        assert_eq!(one, []);
    }

    fn requested_as(id: i64, name: &str) -> RequestedQuality {
        RequestedQuality {
            id,
            name: name.to_owned(),
            profile_id: name.to_owned(),
            ..Default::default()
        }
    }

    const HD: i64 = 1;
    const FOUR_K: i64 = 2;
    const UHD: i64 = 3;

    fn season(id: i64, asked: i64, state: RequestState, handed_off: bool) -> SeasonRequest {
        SeasonRequest {
            id,
            asked: Some(requested_as(asked, &asked.to_string())),
            used: None,
            state,
            handed_off,
        }
    }

    fn show(seasons: Vec<SeasonRequest>, followed: Option<i64>) -> ShowRequests {
        ShowRequests {
            seasons,
            followed: followed.is_some(),
            followed_quality: followed.map(|id| requested_as(id, &id.to_string())),
        }
    }

    fn quality_id(show: &ShowRequests) -> Option<i64> {
        show.quality().map(|quality| quality.id)
    }

    #[test]
    fn a_shows_quality_is_what_its_first_handed_off_season_used() {
        let mut handed_off = season(2, FOUR_K, RequestState::Processing, true);
        handed_off.used = Some(requested_as(UHD, "service's own"));
        let seasons = vec![season(1, HD, RequestState::Pending, false), handed_off];

        assert_eq!(quality_id(&show(seasons, Some(HD))), Some(UHD));
    }

    #[test]
    fn without_a_handed_off_season_a_shows_quality_is_its_earliest_open_seasons_else_its_followed_shows_else_none()
     {
        let closed_first = vec![
            season(1, UHD, RequestState::Declined, false),
            season(2, HD, RequestState::Pending, false),
            season(3, FOUR_K, RequestState::Pending, false),
        ];
        let closed_only = || vec![season(1, UHD, RequestState::Declined, false)];

        assert_eq!(quality_id(&show(closed_first, Some(FOUR_K))), Some(HD));
        assert_eq!(quality_id(&show(closed_only(), Some(FOUR_K))), Some(FOUR_K));
        assert_eq!(quality_id(&show(closed_only(), None)), None);
        assert_eq!(quality_id(&show(Vec::new(), None)), None);
    }

    #[test]
    fn a_shows_quality_is_fixed_once_a_season_is_approved_or_handed_off() {
        let pending = show(vec![season(1, HD, RequestState::Pending, false)], None);
        let approved = show(vec![season(1, HD, RequestState::Approved, false)], None);
        let handed_off = show(vec![season(1, HD, RequestState::Available, true)], None);

        assert!(!pending.is_fixed());
        assert!(approved.is_fixed());
        assert!(handed_off.is_fixed());
    }

    #[test]
    fn a_change_moves_the_open_seasons_not_handed_off_and_the_followed_show() {
        let seasons = vec![
            season(1, HD, RequestState::Declined, false),
            season(2, HD, RequestState::Pending, false),
            season(3, HD, RequestState::Failed, false),
        ];

        let moves = show(seasons, Some(HD)).change(&requested_as(FOUR_K, "4K"));

        assert_eq!(
            moves,
            Ok(Moves {
                seasons: vec![2, 3],
                followed: true
            })
        );
    }

    #[test]
    fn once_fixed_a_change_to_another_quality_is_refused_and_to_the_same_moves_the_rest() {
        let fixed = show(
            vec![
                season(1, FOUR_K, RequestState::Processing, true),
                season(2, FOUR_K, RequestState::Pending, false),
            ],
            None,
        );

        let other = fixed.change(&requested_as(HD, "HD"));
        let same = fixed.change(&requested_as(FOUR_K, "4K"));

        assert_eq!(
            other,
            Err(Refused::ShowQualityFixed {
                named: HD,
                shows: requested_as(FOUR_K, "2"),
            })
        );
        assert_eq!(
            same,
            Ok(Moves {
                seasons: vec![2],
                followed: false
            })
        );
    }

    #[test]
    fn new_seasons_take_the_shows_quality_and_refuse_another_named_one() {
        let shows = show(vec![season(1, FOUR_K, RequestState::Pending, false)], None);
        let asked_hd = |named| Ask {
            taken: Some(requested_as(HD, "HD")),
            chosen: true,
            named,
        };

        let unnamed = shows.for_seasons(asked_hd(None));
        let named = shows.for_seasons(asked_hd(Some(HD)));
        let without_one = show(Vec::new(), None).for_seasons(asked_hd(Some(HD)));

        assert_eq!(
            unnamed,
            Ok(Ask {
                taken: Some(requested_as(FOUR_K, "2")),
                ..Ask::default()
            })
        );
        assert_eq!(
            named,
            Err(Refused::NotTheShows {
                named: HD,
                shows: requested_as(FOUR_K, "2"),
            })
        );
        assert_eq!(without_one, Ok(asked_hd(Some(HD))));
    }

    #[test]
    fn a_joiners_quality_replaces_a_pending_requests_only_if_they_chose_it() {
        let hd = Some(requested_as(HD, "HD"));
        let asked_hd = |chosen| Ask {
            taken: hd.clone(),
            chosen,
            named: None,
        };

        assert_eq!(asked_hd(true).joiner_approves_at(), hd);
        assert_eq!(asked_hd(false).joiner_approves_at(), None);
    }

    fn fulfilled(profile_id: &str, profile_name: &str) -> Fulfilled {
        Fulfilled {
            profile_id: profile_id.to_owned(),
            profile_name: profile_name.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn the_quality_used_is_the_one_asked_else_the_listed_one_else_the_services_own() {
        let asked = requested_as(HD, "4");
        let listed = [
            Quality {
                id: FOUR_K,
                ..quality("5", "4K", false)
            },
            Quality {
                id: HD,
                ..quality("4", "HD", true)
            },
        ];

        let same = used(Some(&asked), &listed, &fulfilled("4", "HD-1080p"));
        let on_the_list = used(Some(&asked), &listed, &fulfilled("5", "Ultra-HD"));
        let off_it = used(Some(&asked), &listed, &fulfilled("9", "Any"));

        assert_eq!(same, Some(asked));
        assert_eq!(
            on_the_list.map(|quality| quality.name),
            Some("4K".to_owned())
        );
        let off_it = off_it.unwrap();
        assert_eq!((off_it.id, off_it.name.as_str()), (0, "Any"));
    }

    #[test]
    fn no_quality_is_used_without_one_asked_or_a_profile_reported() {
        let asked = requested_as(HD, "4");

        assert_eq!(used(None, &[], &fulfilled("4", "HD")), None);
        assert_eq!(used(Some(&asked), &[], &fulfilled("", "")), None);
    }
}
