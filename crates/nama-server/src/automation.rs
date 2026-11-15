//! Automation: the Kernel's functions an Automation Plugin calls (ADR 0048), each checking the
//! Plugin Permission it needs before anything else.

mod store;

use std::{
    collections::HashSet,
    sync::{Arc, Weak},
};

use buffa::{EnumValue, Inline, MessageField};
use nama_plugin_api::{
    BoxFuture, PluginId,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, Libraries, LibrariesAsked, Library, LibraryKind,
        Permission, Refused, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleRef,
        TitleRefKind, TitleWatchStates, Titles, TitlesAsked, WatchState, WatchStates,
        WatchStatesAsked,
    },
    fulfiller::TitleId,
    metadata::ExternalId,
};
use tracing::error;

use crate::{
    browse::{self, Browse, Target, ToDelete},
    db::Database,
    library::{
        contents::Contents,
        libraries::{self, Kind},
        watch_states::{self, MovieOrEpisode},
    },
    plugins::ExtensionPoints,
    scanner::{self, Doomed, Scanner},
};

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
    DeleteMedia,
}

impl Method {
    /// The one place that decides which Permission each function needs.
    const fn needs(self) -> Permission {
        match self {
            Self::StoreGet | Self::StorePut | Self::StoreDelete => Permission::PERMISSION_STORE,
            Self::ListLibraries | Self::ListTitles => Permission::PERMISSION_READ_LIBRARIES,
            Self::ListWatchStates => Permission::PERMISSION_READ_WATCH_STATES,
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
    fn check(&self, method: Method) -> Result<Arc<Automation>, Refused> {
        let needs = method.needs();
        if !self.granted.contains(&needs) {
            return Err(Refused(format!(
                "the Admin hasn't granted {} the {} Permission",
                self.plugin,
                permission_name(needs)
            )));
        }
        (self.automation.upgrade()).ok_or_else(|| Refused("the Server is stopping".to_owned()))
    }

    /// Refuses with the Store's reason, or, for a database failure, logs it and refuses without it.
    fn store_refused(&self, err: &store::Error) -> Refused {
        if let store::Error::Database(err) = err {
            error!(plugin = %self.plugin, "the Store failed: {err}");
            return Refused("the Kernel couldn't reach the Store".to_owned());
        }
        Refused(err.to_string())
    }

    /// Logs a database failure while `doing` something, and refuses without its cause.
    fn database_refused(&self, doing: &str, err: &impl std::fmt::Display) -> Refused {
        error!(plugin = %self.plugin, "{doing} failed: {err}");
        Refused(format!("the Kernel failed {doing}"))
    }

    /// The Matched Movies, or Episodes, of the Library `library`.
    async fn titles(&self, automation: &Automation, library: i64) -> Result<Vec<Title>, Refused> {
        let found = libraries::Libraries::new(Arc::clone(&automation.db))
            .get(library)
            .await;
        let kind = match found {
            Ok(found) => found.kind,
            Err(libraries::Error::Database(err)) => {
                return Err(self.database_refused("reading the Library", &err));
            }
            Err(err) => return Err(Refused(err.to_string())),
        };
        let contents = Contents::new(Arc::clone(&automation.db));
        let read = |err| self.database_refused("reading the Library's titles", &err);
        let titles = match kind {
            Kind::Movies => (contents.movies(library).await.map_err(read)?.into_iter())
                .map(|movie| Title {
                    title_ref: title_ref(library, TitleRefKind::TITLE_REF_KIND_MOVIE, &movie.id),
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
                        title_ref: title_ref(
                            library,
                            TitleRefKind::TITLE_REF_KIND_EPISODE,
                            &episode.id,
                        ),
                        title: episode.title,
                        show_title: Some(show.title.clone()),
                        season_number: u32::try_from(episode.season).ok(),
                        episode_number: u32::try_from(episode.number).ok(),
                        ..Title::default()
                    })
                })
                .collect(),
        };
        Ok(titles)
    }
}

/// `id` of a Movie or Episode of the Library `library`, as a Plugin names it.
fn title_ref(
    library: i64,
    kind: TitleRefKind,
    id: &ExternalId,
) -> MessageField<TitleRef, Inline<TitleRef>> {
    MessageField::some(TitleRef {
        library_id: library,
        kind: kind.into(),
        id: MessageField::some(TitleId {
            namespace: id.namespace().to_owned(),
            value: id.value().to_owned(),
            ..TitleId::default()
        }),
        ..TitleRef::default()
    })
}

/// The Movie or Episode `title` names, as `movie(id)` or `episode(id)`, or why it names none.
fn titled<T>(
    title: &TitleRef,
    movie: impl FnOnce(ExternalId) -> T,
    episode: impl FnOnce(ExternalId) -> T,
) -> Result<T, Refused> {
    let id = title
        .id
        .as_option()
        .ok_or_else(|| Refused("the title's id is missing".to_owned()))?;
    let id = ExternalId::new(&id.namespace, id.value.clone())
        .map_err(|err| Refused(format!("the title's id is malformed: {err}")))?;
    match title.kind {
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_MOVIE) => Ok(movie(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_EPISODE) => Ok(episode(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            Refused("the kind of title must be Movie or Episode".to_owned()),
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
    fn store_get<'a>(&'a self, key: &'a StoreKey) -> BoxFuture<'a, Result<StoreValue, Refused>> {
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

    fn store_put<'a>(&'a self, entry: &'a StoreEntry) -> BoxFuture<'a, Result<Stored, Refused>> {
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

    fn store_delete<'a>(&'a self, key: &'a StoreKey) -> BoxFuture<'a, Result<Stored, Refused>> {
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
    ) -> BoxFuture<'a, Result<Libraries, Refused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListLibraries)?;
            let listed = libraries::Libraries::new(Arc::clone(&automation.db))
                .list()
                .await
                .map_err(|err| {
                    error!(plugin = %self.plugin, "listing the Libraries failed: {err}");
                    Refused("the Kernel couldn't read the Libraries".to_owned())
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

    fn list_titles<'a>(&'a self, asked: &'a TitlesAsked) -> BoxFuture<'a, Result<Titles, Refused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListTitles)?;
            Ok(Titles {
                titles: self.titles(&automation, asked.library_id).await?,
                ..Titles::default()
            })
        })
    }

    fn list_watch_states<'a>(
        &'a self,
        asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, Refused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListWatchStates)?;
            let titles = (asked.titles.iter())
                .map(|title| {
                    titled(
                        title,
                        |id| MovieOrEpisode::movie(&id),
                        |id| MovieOrEpisode::episode(&id),
                    )
                })
                .collect::<Result<Vec<_>, Refused>>()?;
            let states = watch_states::WatchStates::new(Arc::clone(&automation.db))
                .of_every_user(&titles)
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

    fn delete_media<'a>(
        &'a self,
        asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, Refused>> {
        Box::pin(async move {
            let automation = self.check(Method::DeleteMedia)?;
            let title = (asked.title_ref.as_option())
                .ok_or_else(|| Refused("the title to delete is missing".to_owned()))?;
            let to_delete = ToDelete::Title {
                library: title.library_id,
                title: titled(title, Target::Movie, Target::Episode)?,
            };
            let doomed = match automation.browse.doomed(to_delete).await {
                Ok(doomed) => doomed,
                Err(browse::Error::Database(err)) => {
                    return Err(self.database_refused("finding the media to delete", &err));
                }
                Err(err) => return Err(Refused(err.to_string())),
            };
            if let Some(folder) = doomed.read_only_folder() {
                return Err(Refused(
                    scanner::Error::ReadOnly(folder.to_owned()).to_string(),
                ));
            }
            let by = format!("the {} Plugin", automation.points.name(&self.plugin));
            // Not awaited: the delete waits for the Library's scan lock, which could outrun the
            // Plugin's call limit, and it goes on in a task of its own anyway (ADR 0029).
            tokio::spawn(delete(
                automation.scanner.clone(),
                doomed,
                by,
                self.plugin.clone(),
            ));
            Ok(DeleteStarted::default())
        })
    }
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
