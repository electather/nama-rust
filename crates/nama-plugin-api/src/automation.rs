//! The Kernel's functions, which any Plugin may call through the [`Kernel`] it finds in
//! [`Registrar::kernel`](crate::Registrar::kernel) (ADR 0048, ADR 0071).

use std::{error::Error, fmt};

pub use nama_plugin_proto::nama::plugin::v1::{
    DeleteAsked, DeleteStarted, ExpiredSignInsAsked, ExpiredSignInsDeleted, ForgetGonePluginsAsked,
    ForgetHomeRowsAsked, GonePluginsForgotten, HomeRowsForgotten, Libraries, LibrariesAsked,
    Library, LibraryKind, Permission, Requesters, RequestersAsked, StoreEntry, StoreKey,
    StoreValue, Stored, Title, TitleRef, TitleRefKind, TitleRequesters, TitleWatchStates, Titles,
    TitlesAsked, User, Users, UsersAsked, WatchState, WatchStates, WatchStatesAsked, Watchlist,
    WatchlistAsked, WatchlistTitle,
};

use crate::BoxFuture;

/// Why the [`Kernel`] refused a function, in plain words, such as a Permission not granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRefused(pub String);

impl fmt::Display for KernelRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for KernelRefused {}

/// What any Plugin may ask the Kernel: one of its functions by name, with the request
/// `automation.proto` names for it encoded. Each function needs the Plugin [`Permission`]
/// `automation.proto` names beside it, which the Kernel checks. A closure is one, so a test's fake
/// Kernel is a closure.
pub trait Kernel: Send + Sync + 'static {
    /// Asks the Kernel's function `function`, and answers its encoded response.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, in plain words: such as a function it doesn't have, a Plugin
    /// Permission not granted, or a request it can't read.
    fn call(
        &self,
        function: &str,
        request: Vec<u8>,
    ) -> BoxFuture<'_, Result<Vec<u8>, KernelRefused>>;
}

impl<F> Kernel for F
where
    F: Fn(&str, Vec<u8>) -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>>
        + Send
        + Sync
        + 'static,
{
    fn call(
        &self,
        function: &str,
        request: Vec<u8>,
    ) -> BoxFuture<'_, Result<Vec<u8>, KernelRefused>> {
        self(function, request)
    }
}
