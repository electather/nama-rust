//! Provide: a Home row source, which gives headed rows of titles for the home page as data the
//! Kernel draws (ADR 0058).
//!
//! A Plugin serves it by registering an `Arc<dyn HomeRowSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn HomeRowSource>(Priority::DEFAULT, Arc::new(my_rows));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{HomeRows, SharedRowsAsked, UserRowsAsked},
};

/// A Home row source. It speaks the contract's `HomeRowService` messages, as an Installed Plugin's
/// exports do. The Kernel asks it ahead of time and stores the answer, so home never waits on it.
///
/// Both methods are optional: one left out gives no rows, so one point serves sources that give
/// only shared rows, only each User's, or both.
pub trait HomeRowSource: Send + Sync + 'static {
    /// The rows every User sees, in the order home shows them.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps the rows stored before.
    fn shared_rows<'a>(
        &'a self,
        _asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        no_rows()
    }

    /// One User's own rows, in the order home shows them, from that User's Personal settings in
    /// `asked`.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps that User's rows stored before.
    fn user_rows<'a>(
        &'a self,
        _asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        no_rows()
    }

    /// Whether this source gives the rows `method` asks for. The Kernel asks it only for those;
    /// a Built-in that leaves a method out may keep the default, which costs a call giving no
    /// rows each time those rows go stale, every 15 minutes for each User who opens home.
    fn gives(&self, _method: HomeRowMethod) -> bool {
        true
    }
}

/// A [`HomeRowSource`] method the Kernel may ask, for [`HomeRowSource::gives`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomeRowMethod {
    /// [`HomeRowSource::shared_rows`].
    Shared,
    /// [`HomeRowSource::user_rows`].
    User,
}

/// The answer of a [`HomeRowSource`] method its source left out.
fn no_rows<'a>() -> BoxFuture<'a, Result<HomeRows, PluginError>> {
    Box::pin(std::future::ready(Ok(HomeRows::default())))
}
