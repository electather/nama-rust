//! Provide: the Automation lane, where a Plugin asks the Kernel (ADR 0048).
//!
//! The Kernel ticks an [`Automation`] on the schedule its [`Manifest`](crate::Manifest) sets in
//! `tick_every`, and an Automation Plugin may call the [`Kernel`] it finds in
//! [`Registrar::kernel`](crate::Registrar::kernel). A Plugin provides one by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn Automation>(Priority::DEFAULT, Arc::new(my_automation));
//! ```

use std::{error::Error, fmt};

pub use nama_plugin_proto::nama::plugin::v1::{
    DeleteAsked, DeleteStarted, Libraries, LibrariesAsked, Library, LibraryKind, Permission,
    Requesters, RequestersAsked, StoreEntry, StoreKey, StoreValue, Stored, Tick, Title, TitleRef,
    TitleRefKind, TitleRequesters, TitleWatchStates, Titles, TitlesAsked, User, Users, UsersAsked,
    WatchState, WatchStates, WatchStatesAsked,
};

use crate::{BoxFuture, PluginError};

/// A Provider the Kernel ticks on the schedule its Plugin's Manifest sets.
pub trait Automation: Send + Sync + 'static {
    /// Does the Plugin's scheduled work. The Kernel logs a failure and counts it as an Observer's.
    ///
    /// # Errors
    ///
    /// When the work failed; the Kernel logs why.
    fn tick<'a>(&'a self, tick: &'a Tick) -> BoxFuture<'a, Result<(), PluginError>>;
}

/// Why the [`Kernel`] refused a function, in plain words, such as a Permission not granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRefused(pub String);

impl fmt::Display for KernelRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for KernelRefused {}

/// What an Automation Plugin may ask the Kernel. Each method needs the Plugin [`Permission`]
/// `automation.proto` names beside it, which the Kernel checks.
pub trait Kernel: Send + Sync + 'static {
    /// The value kept under `key` in the Plugin's Store. Needs `PERMISSION_STORE`.
    fn store_get<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<StoreValue, KernelRefused>>;

    /// Keeps `entry` in the Plugin's Store, replacing any value under its key and User, and
    /// refuses a new key once the Plugin keeps 10 000 entries. Needs `PERMISSION_STORE`.
    fn store_put<'a>(
        &'a self,
        entry: &'a StoreEntry,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>>;

    /// Removes the value kept under `key` in the Plugin's Store. Needs `PERMISSION_STORE`.
    fn store_delete<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>>;

    /// Every Library. Needs `PERMISSION_READ_LIBRARIES`.
    fn list_libraries<'a>(
        &'a self,
        asked: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, KernelRefused>>;

    /// A page of a Library's Movies, or its Shows' Episodes, by kind and then id, or every one
    /// for a `page_size` of 0. Needs `PERMISSION_READ_LIBRARIES`.
    fn list_titles<'a>(
        &'a self,
        asked: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, KernelRefused>>;

    /// Every User's Watch State of each title asked for. Needs `PERMISSION_READ_WATCH_STATES`.
    fn list_watch_states<'a>(
        &'a self,
        asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, KernelRefused>>;

    /// Every User, with their id and display name only. Needs `PERMISSION_READ_USERS`.
    fn list_users<'a>(
        &'a self,
        asked: &'a UsersAsked,
    ) -> BoxFuture<'a, Result<Users, KernelRefused>>;

    /// Who Requested each title asked for, as ids only. Needs `PERMISSION_READ_REQUESTS`.
    fn list_requesters<'a>(
        &'a self,
        asked: &'a RequestersAsked,
    ) -> BoxFuture<'a, Result<Requesters, KernelRefused>>;

    /// Starts deleting a title's media files, once the Kernel checked the delete, and answers the
    /// other titles its files take along. Refuses, deleting nothing, one taking along an Episode
    /// `asked.also` doesn't name. Needs `PERMISSION_DELETE_MEDIA`.
    fn delete_media<'a>(
        &'a self,
        asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, KernelRefused>>;
}
