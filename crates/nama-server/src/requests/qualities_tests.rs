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
