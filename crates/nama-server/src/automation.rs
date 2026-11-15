//! Automation: the Kernel's functions an Automation Plugin calls (ADR 0048), each checking the
//! Plugin Permission it needs before anything else.

mod store;

use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::{
    BoxFuture, PluginId,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, Libraries, LibrariesAsked, Library, LibraryKind,
        Permission, Refused, StoreEntry, StoreKey, StoreValue, Stored, Titles, TitlesAsked,
        WatchStates, WatchStatesAsked,
    },
};
use tracing::error;

use crate::{
    db::Database,
    library::libraries::{self, Kind},
};

/// What makes the Kernel's functions for each enabled Automation Plugin.
pub(crate) struct Automation {
    db: Arc<Database>,
}

impl Automation {
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// The Kernel's functions as `plugin` calls them, granted `granted`.
    pub(crate) fn kernel(&self, plugin: PluginId, granted: HashSet<Permission>) -> Arc<dyn Kernel> {
        Arc::new(PluginKernel {
            db: Arc::clone(&self.db),
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
    db: Arc<Database>,
    plugin: PluginId,
    granted: HashSet<Permission>,
}

impl PluginKernel {
    fn check(&self, method: Method) -> Result<(), Refused> {
        let needs = method.needs();
        if self.granted.contains(&needs) {
            return Ok(());
        }
        Err(Refused(format!(
            "the Admin hasn't granted {} the {} Permission",
            self.plugin,
            permission_name(needs)
        )))
    }

    /// Refuses with the Store's reason, or, for a database failure, logs it and refuses without it.
    fn store_refused(&self, err: &store::Error) -> Refused {
        if let store::Error::Database(err) = err {
            error!(plugin = %self.plugin, "the Store failed: {err}");
            return Refused("the Kernel couldn't reach the Store".to_owned());
        }
        Refused(err.to_string())
    }
}

/// What a function not served yet answers, once its Permission is checked.
fn not_served() -> Refused {
    Refused("not served yet".to_owned())
}

impl Kernel for PluginKernel {
    fn store_get<'a>(&'a self, key: &'a StoreKey) -> BoxFuture<'a, Result<StoreValue, Refused>> {
        Box::pin(async move {
            self.check(Method::StoreGet)?;
            let value = store::get(&self.db, &self.plugin, key.user_id, &key.key)
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
            self.check(Method::StorePut)?;
            store::put(
                &self.db,
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
            self.check(Method::StoreDelete)?;
            store::delete(&self.db, &self.plugin, key.user_id, key.key.clone())
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
            self.check(Method::ListLibraries)?;
            let listed = libraries::Libraries::new(Arc::clone(&self.db))
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

    fn list_titles<'a>(
        &'a self,
        _asked: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, Refused>> {
        Box::pin(async move {
            self.check(Method::ListTitles)?;
            Err(not_served())
        })
    }

    fn list_watch_states<'a>(
        &'a self,
        _asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, Refused>> {
        Box::pin(async move {
            self.check(Method::ListWatchStates)?;
            Err(not_served())
        })
    }

    fn delete_media<'a>(
        &'a self,
        _asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, Refused>> {
        Box::pin(async move {
            self.check(Method::DeleteMedia)?;
            Err(not_served())
        })
    }
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
