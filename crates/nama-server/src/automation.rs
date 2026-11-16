//! Automation: the Kernel's functions an Automation Plugin calls (ADR 0048), each checking the
//! Plugin Permission it needs before anything else.

mod store;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
};

use buffa::{EnumValue, MessageField};
use nama_plugin_api::{
    BoxFuture, PluginId,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, KernelRefused, Libraries, LibrariesAsked, Library,
        LibraryKind, Permission, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleRef,
        TitleRefKind, TitleWatchStates, Titles, TitlesAsked, User, Users, UsersAsked, WatchState,
        WatchStates, WatchStatesAsked,
    },
    fulfiller::TitleId,
    metadata::{ExternalId, ExternalIds},
};
use tracing::error;

use crate::{
    browse::{self, Browse, Target, ToDelete},
    db::Database,
    library::{
        contents::{Contents, ItemKind},
        libraries::{self, Kind},
        watch_states,
    },
    plugins::ExtensionPoints,
    scanner::{self, Doomed, Scanner},
    users,
};

/// The most titles one page of `list_titles` holds.
const MAX_PAGE_SIZE: usize = 1000;

/// What makes the Kernel's functions for each enabled Automation Plugin.
///
/// The Plugins it serves hold it only weakly, since it holds them in turn through the Scanner and
/// the Extension Points: whoever starts the Server holds it, until the Plugins have stopped.
pub(crate) struct Automation {
    db: Arc<Database>,
    browse: Browse,
    scanner: Scanner,
    points: ExtensionPoints,
}

impl Automation {
    /// Serves the Kernel's functions to each Automation Plugin `points` enables.
    pub(crate) fn serve(db: Arc<Database>, scanner: Scanner, points: ExtensionPoints) -> Arc<Self> {
        let automation = Arc::new(Self {
            browse: Browse::new(Arc::clone(&db)),
            db,
            scanner,
            points,
        });
        let kernels = Arc::downgrade(&automation);
        automation
            .points
            .serve_kernel(Arc::new(move |plugin, granted| {
                Self::kernel(&kernels, plugin, granted)
            }));
        automation
    }

    /// The Kernel's functions as `plugin` calls them, granted `granted`. Each refuses once
    /// `automation` is gone, as the Server stops.
    fn kernel(
        automation: &Weak<Self>,
        plugin: PluginId,
        granted: HashSet<Permission>,
    ) -> Arc<dyn Kernel> {
        Arc::new(PluginKernel {
            automation: Weak::clone(automation),
            plugin,
            granted,
        })
    }
}

/// A function of the Kernel's, for the one table of what each needs.
#[derive(Debug, Clone, Copy)]
enum Method {
    StoreGet,
    StorePut,
    StoreDelete,
    ListLibraries,
    ListTitles,
    ListWatchStates,
    ListUsers,
    DeleteMedia,
}

impl Method {
    /// The one place that decides which Permission each function needs.
    const fn needs(self) -> Permission {
        match self {
            Self::StoreGet | Self::StorePut | Self::StoreDelete => Permission::PERMISSION_STORE,
            Self::ListLibraries | Self::ListTitles => Permission::PERMISSION_READ_LIBRARIES,
            Self::ListWatchStates => Permission::PERMISSION_READ_WATCH_STATES,
            Self::ListUsers => Permission::PERMISSION_READ_USERS,
            Self::DeleteMedia => Permission::PERMISSION_DELETE_MEDIA,
        }
    }
}

/// How an Admin reads a Permission's name.
const fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::PERMISSION_UNSPECIFIED => "unnamed",
        Permission::PERMISSION_STORE => "Store",
        Permission::PERMISSION_READ_LIBRARIES => "Read Libraries",
        Permission::PERMISSION_READ_WATCH_STATES => "Read Watch States",
        Permission::PERMISSION_DELETE_MEDIA => "Delete media",
        Permission::PERMISSION_READ_USERS => "Read Users",
    }
}

struct PluginKernel {
    /// Held weakly: see [`Automation`].
    automation: Weak<Automation>,
    plugin: PluginId,
    granted: HashSet<Permission>,
}

impl PluginKernel {
    /// Checks the Plugin was granted what `method` needs, then hands out the Automation.
    fn check(&self, method: Method) -> Result<Arc<Automation>, KernelRefused> {
        let needs = method.needs();
        if !self.granted.contains(&needs) {
            return Err(KernelRefused(format!(
                "the Admin hasn't granted {} the {} Permission",
                self.plugin,
                permission_name(needs)
            )));
        }
        (self.automation.upgrade())
            .ok_or_else(|| KernelRefused("the Server is stopping".to_owned()))
    }

    /// Refuses with the Store's reason, or, for a database failure, logs it and refuses without it.
    fn store_refused(&self, err: &store::Error) -> KernelRefused {
        if let store::Error::Database(err) = err {
            error!(plugin = %self.plugin, "the Store failed: {err}");
            return KernelRefused("the Kernel couldn't reach the Store".to_owned());
        }
        KernelRefused(err.to_string())
    }

    /// Logs a database failure while `doing` something, and refuses without its cause.
    fn database_refused(&self, doing: &str, err: &impl std::fmt::Display) -> KernelRefused {
        error!(plugin = %self.plugin, "{doing} failed: {err}");
        KernelRefused(format!("the Kernel failed {doing}"))
    }

    /// The Matched Movies, or Episodes, of the Library `library`, by kind and then id.
    // ponytail: whole-Library read, even for one page of `list_titles`.
    async fn titles(
        &self,
        automation: &Automation,
        library: i64,
    ) -> Result<Vec<Title>, KernelRefused> {
        let found = libraries::Libraries::new(Arc::clone(&automation.db))
            .get(library)
            .await;
        let kind = match found {
            Ok(found) => found.kind,
            Err(libraries::Error::Database(err)) => {
                return Err(self.database_refused("reading the Library", &err));
            }
            Err(err) => return Err(KernelRefused(err.to_string())),
        };
        let contents = Contents::new(Arc::clone(&automation.db));
        let read = |err| self.database_refused("reading the Library's titles", &err);
        let titles = match kind {
            Kind::Movies => (contents.movies(library).await.map_err(read)?.into_iter())
                .map(|movie| Title {
                    title_ref: MessageField::some(title_ref(
                        library,
                        TitleRefKind::TITLE_REF_KIND_MOVIE,
                        &movie.id,
                    )),
                    title: movie.title,
                    ..Title::default()
                })
                .collect(),
            Kind::Shows => (contents.shows(library).await.map_err(read)?.into_iter())
                .flat_map(|show| {
                    let episodes = show
                        .seasons
                        .into_values()
                        .flat_map(|season| season.episodes);
                    episodes.map(move |episode| Title {
                        title_ref: MessageField::some(title_ref(
                            library,
                            TitleRefKind::TITLE_REF_KIND_EPISODE,
                            &episode.id,
                        )),
                        title: episode.title,
                        show_title: Some(show.title.clone()),
                        season_number: u32::try_from(episode.season).ok(),
                        episode_number: u32::try_from(episode.number).ok(),
                        ..Title::default()
                    })
                })
                .collect(),
        };
        let mut titles: Vec<Title> = titles;
        titles.sort_by_cached_key(|title| {
            let id = title.title_ref.id.as_option();
            (
                title.title_ref.kind.to_i32(),
                id.map(|id| id.namespace.clone()),
                id.map(|id| id.value.clone()),
            )
        });
        Ok(titles)
    }

    /// The page of the Library's titles `asked` names.
    async fn titles_page(
        &self,
        automation: &Automation,
        asked: &TitlesAsked,
    ) -> Result<Titles, KernelRefused> {
        let start = match asked.page_token.as_str() {
            "" => 0,
            token => token.parse::<usize>().map_err(|_malformed| {
                KernelRefused(format!(
                    "the page token {token:?} isn't one the Kernel handed out"
                ))
            })?,
        };
        let mut titles = self.titles(automation, asked.library_id).await?;
        if asked.page_size == 0 {
            return Ok(Titles {
                titles,
                ..Titles::default()
            });
        }
        let size = usize::try_from(asked.page_size)
            .unwrap_or(MAX_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let end = start.saturating_add(size);
        let next_page_token = if end < titles.len() {
            end.to_string()
        } else {
            String::new()
        };
        titles.truncate(end);
        let page = titles.split_off(start.min(titles.len()));
        Ok(Titles {
            titles: page,
            next_page_token,
            ..Titles::default()
        })
    }

    /// Each of `titles` as a Watch State kind and every id its Library's Match gives it, any of
    /// which its Watch State may be kept under: the preferred id follows each Library's title
    /// source order (ADR 0042). A title its Library doesn't have keeps only the id it is named by.
    async fn every_id(
        &self,
        automation: &Automation,
        titles: &[TitleRef],
    ) -> Result<Vec<(watch_states::Kind, ExternalIds)>, KernelRefused> {
        let contents = Contents::new(Arc::clone(&automation.db));
        let read = |err| self.database_refused("reading the Library's titles", &err);
        // One read per Library and kind of the titles asked about, each answer going back to the
        // place its title was asked in.
        let mut asked: HashMap<(i64, watch_states::Kind), Vec<(usize, ExternalId)>> =
            HashMap::new();
        for (place, title) in titles.iter().enumerate() {
            let (kind, named) = titled(
                title,
                |id| (watch_states::Kind::Movie, id),
                |id| (watch_states::Kind::Episode, id),
            )?;
            (asked.entry((title.library_id, kind)).or_default()).push((place, named));
        }
        let mut every_id = Vec::with_capacity(titles.len());
        for ((library, kind), named) in asked {
            let (places, named): (Vec<usize>, Vec<ExternalId>) = named.into_iter().unzip();
            let item_kind = match kind {
                watch_states::Kind::Movie => ItemKind::Movie,
                watch_states::Kind::Episode => ItemKind::Episode,
            };
            let found = (contents.every_id(library, item_kind, &named).await).map_err(read)?;
            every_id
                .extend((places.into_iter().zip(found)).map(|(place, ids)| (place, (kind, ids))));
        }
        every_id.sort_unstable_by_key(|&(place, _)| place);
        Ok(every_id.into_iter().map(|(_, title)| title).collect())
    }

    /// Refuses a delete that would take along an Episode `also` doesn't name, naming each one.
    async fn check_taken_along(
        &self,
        automation: &Automation,
        library: i64,
        taken_along: &[TitleRef],
        also: &[TitleRef],
    ) -> Result<(), KernelRefused> {
        let unnamed: Vec<&TitleRef> = (taken_along.iter())
            .filter(|title| !also.iter().any(|named| same_title(named, title)))
            .collect();
        if unnamed.is_empty() {
            return Ok(());
        }
        // Only for the refusal, so the Plugin can tell which Episodes they are.
        let titles = self.titles(automation, library).await?;
        let named: Vec<String> = (unnamed.into_iter())
            .map(|unnamed| {
                (titles.iter())
                    .find(|title| same_title(&title.title_ref, unnamed))
                    .map_or_else(|| "an Episode".to_owned(), episode_name)
            })
            .collect();
        Err(KernelRefused(format!(
            "the title's files also hold {}; name each in `also` to delete it too, or keep the \
             files",
            named.join(", ")
        )))
    }
}

/// Whether `a` and `b` name the same title of the same Library.
fn same_title(a: &TitleRef, b: &TitleRef) -> bool {
    let id = |title: &TitleRef| {
        (title.id.as_option()).map(|id| (id.namespace.clone(), id.value.clone()))
    };
    a.library_id == b.library_id && a.kind == b.kind && id(a) == id(b)
}

/// An Episode as a refusal names it, such as `Dark S01E02 (Lies)`.
fn episode_name(episode: &Title) -> String {
    format!(
        "{} S{:02}E{:02} ({})",
        episode.show_title.as_deref().unwrap_or_default(),
        episode.season_number.unwrap_or_default(),
        episode.episode_number.unwrap_or_default(),
        episode.title
    )
}

/// `id` of a Movie or Episode of the Library `library`, as a Plugin names it.
fn title_ref(library: i64, kind: TitleRefKind, id: &ExternalId) -> TitleRef {
    TitleRef {
        library_id: library,
        kind: kind.into(),
        id: MessageField::some(TitleId {
            namespace: id.namespace().to_owned(),
            value: id.value().to_owned(),
            ..TitleId::default()
        }),
        ..TitleRef::default()
    }
}

/// The Movie or Episode `title` names, as `movie(id)` or `episode(id)`, or why it names none.
fn titled<T>(
    title: &TitleRef,
    movie: impl FnOnce(ExternalId) -> T,
    episode: impl FnOnce(ExternalId) -> T,
) -> Result<T, KernelRefused> {
    let id = title
        .id
        .as_option()
        .ok_or_else(|| KernelRefused("the title's id is missing".to_owned()))?;
    let id = ExternalId::new(&id.namespace, id.value.clone())
        .map_err(|err| KernelRefused(format!("the title's id is malformed: {err}")))?;
    match title.kind {
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_MOVIE) => Ok(movie(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_EPISODE) => Ok(episode(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            KernelRefused("the kind of title must be Movie or Episode".to_owned()),
        ),
    }
}

/// Deletes `doomed` for `plugin`, logging a failure, since the Plugin has had its answer.
async fn delete(scanner: Scanner, doomed: Doomed, by: String, plugin: PluginId) {
    if let Err(err) = scanner.delete_media(doomed, by).await {
        error!(%plugin, "the Plugin's delete failed: {err}");
    }
}

/// What `state`, kept for the User `user`, is to a Plugin.
fn watch_state_to_proto(user: i64, state: watch_states::WatchState) -> WatchState {
    WatchState {
        user_id: user,
        played: state.played,
        position_ms: u64::try_from(state.position_ms).unwrap_or_default(),
        duration_ms: u64::try_from(state.duration_ms).unwrap_or_default(),
        updated_at_ms: state.updated_at.saturating_mul(1000),
        ..WatchState::default()
    }
}

impl Kernel for PluginKernel {
    fn store_get<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<StoreValue, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StoreGet)?;
            let value = store::get(&automation.db, &self.plugin, key.user_id, &key.key)
                .await
                .map_err(|err| self.store_refused(&err))?;
            Ok(StoreValue {
                value,
                ..StoreValue::default()
            })
        })
    }

    fn store_put<'a>(
        &'a self,
        entry: &'a StoreEntry,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StorePut)?;
            store::put(
                &automation.db,
                &self.plugin,
                entry.user_id,
                entry.key.clone(),
                entry.value.clone(),
            )
            .await
            .map_err(|err| self.store_refused(&err))?;
            Ok(Stored::default())
        })
    }

    fn store_delete<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StoreDelete)?;
            store::delete(&automation.db, &self.plugin, key.user_id, key.key.clone())
                .await
                .map_err(|err| self.store_refused(&err))?;
            Ok(Stored::default())
        })
    }

    fn list_libraries<'a>(
        &'a self,
        _asked: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListLibraries)?;
            let listed = libraries::Libraries::new(Arc::clone(&automation.db))
                .list()
                .await
                .map_err(|err| {
                    error!(plugin = %self.plugin, "listing the Libraries failed: {err}");
                    KernelRefused("the Kernel couldn't read the Libraries".to_owned())
                })?;
            Ok(Libraries {
                libraries: listed
                    .into_iter()
                    .map(|library| Library {
                        id: library.id,
                        name: library.name,
                        kind: match library.kind {
                            Kind::Movies => LibraryKind::LIBRARY_KIND_MOVIES,
                            Kind::Shows => LibraryKind::LIBRARY_KIND_SHOWS,
                        }
                        .into(),
                        ..Library::default()
                    })
                    .collect(),
                ..Libraries::default()
            })
        })
    }

    fn list_titles<'a>(
        &'a self,
        asked: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListTitles)?;
            self.titles_page(&automation, asked).await
        })
    }

    fn list_watch_states<'a>(
        &'a self,
        asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListWatchStates)?;
            let titles = self.every_id(&automation, &asked.titles).await?;
            let states = (watch_states::WatchStates::new(Arc::clone(&automation.db)))
                .of_titles(&titles)
                .await
                .map_err(|err| self.database_refused("reading the Watch States", &err))?;
            Ok(WatchStates {
                titles: (asked.titles.iter().zip(states))
                    .map(|(title, states)| TitleWatchStates {
                        title_ref: MessageField::some(title.clone()),
                        states: (states.into_iter())
                            .map(|(user, state)| watch_state_to_proto(user, state))
                            .collect(),
                        ..TitleWatchStates::default()
                    })
                    .collect(),
                ..WatchStates::default()
            })
        })
    }

    fn list_users<'a>(
        &'a self,
        _asked: &'a UsersAsked,
    ) -> BoxFuture<'a, Result<Users, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListUsers)?;
            let listed = users::Users::new(Arc::clone(&automation.db), automation.points.clone())
                .list()
                .await
                .map_err(|err| self.database_refused("reading the Users", &err))?;
            Ok(Users {
                users: (listed.into_iter())
                    .map(|user| User {
                        id: user.id,
                        display_name: user.name,
                        ..User::default()
                    })
                    .collect(),
                ..Users::default()
            })
        })
    }

    fn delete_media<'a>(
        &'a self,
        asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::DeleteMedia)?;
            let title = (asked.title_ref.as_option())
                .ok_or_else(|| KernelRefused("the title to delete is missing".to_owned()))?;
            let to_delete = ToDelete::Title {
                library: title.library_id,
                title: titled(title, Target::Movie, Target::Episode)?,
            };
            let doomed = match automation.browse.doomed(to_delete).await {
                Ok(doomed) => doomed,
                Err(browse::Error::Database(err)) => {
                    return Err(self.database_refused("finding the media to delete", &err));
                }
                Err(err) => return Err(KernelRefused(err.to_string())),
            };
            if let Some(folder) = doomed.read_only_folder() {
                return Err(KernelRefused(
                    scanner::Error::ReadOnly(folder.to_owned()).to_string(),
                ));
            }
            let taken_along: Vec<TitleRef> = (doomed.taken_along.iter())
                .map(|episode| {
                    let kind = TitleRefKind::TITLE_REF_KIND_EPISODE;
                    title_ref(title.library_id, kind, &episode.id)
                })
                .collect();
            (self.check_taken_along(&automation, title.library_id, &taken_along, &asked.also))
                .await?;
            let by = format!("the {} Plugin", automation.points.name(&self.plugin));
            // Not awaited: the delete waits for the Library's scan lock, which could outrun the
            // Plugin's call limit, and it goes on in a task of its own anyway (ADR 0029). Outside
            // the Plugin's scope, since it never calls the Plugin back, so disabling the Plugin
            // doesn't stop a delete it started (ADR 0048).
            tokio::spawn(delete(
                automation.scanner.clone(),
                doomed,
                by,
                self.plugin.clone(),
            ));
            Ok(DeleteStarted {
                taken_along,
                ..DeleteStarted::default()
            })
        })
    }
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
