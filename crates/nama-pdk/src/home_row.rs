//! A Home row source: gives headed rows of titles for the home page, which the Server draws.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{HomeRow, HomeRowTitle, HomeRows, UserRowsAsked};

/// A Home row source (ADR 0058). Its Manifest's `points` name
/// [`services::HOME_ROW`](crate::services::HOME_ROW). Both methods are optional, and only those
/// [`home_row_source!`](crate::home_row_source!) lists are exported; the Server asks only for
/// those, so a source giving only shared rows is never asked for a User's.
///
/// The Server asks ahead of time and stores the answer, so home never waits on the Plugin. It
/// keeps the first 3 rows of each answer, the first 200 titles of each and 40 characters of each
/// heading, clamps each time to live to between 15 minutes and 1 week, and shows only the titles
/// on the Server:
///
/// ```
/// use nama_pdk::{
///     HomeRow, HomeRowSource, HomeRowTitle, HomeRows, TitleId, TitleKind, UserRowsAsked,
///     extism_pdk::Error,
/// };
///
/// struct Classics;
///
/// impl HomeRowSource for Classics {
///     fn shared_rows() -> Result<HomeRows, Error> {
///         Ok(rows("classics", "Classics"))
///     }
///
///     // A User's own row, headed by the name they set in the Plugin's Personal settings.
///     fn user_rows(asked: UserRowsAsked) -> Result<HomeRows, Error> {
///         let Some(name) = asked.personal_settings.get("name") else {
///             return Ok(HomeRows::default());
///         };
///         Ok(rows("yours", &format!("Classics for {name}")))
///     }
/// }
///
/// fn rows(key: &str, heading: &str) -> HomeRows {
///     let alien = HomeRowTitle {
///         kind: TitleKind::TITLE_KIND_MOVIE.into(),
///         id: TitleId {
///             namespace: "tmdb".to_owned(),
///             value: "348".to_owned(),
///             ..TitleId::default()
///         }
///         .into(),
///         ..HomeRowTitle::default()
///     };
///     let row = HomeRow {
///         key: key.to_owned(),
///         heading: heading.to_owned(),
///         ttl_seconds: 24 * 60 * 60,
///         titles: vec![alien],
///         ..HomeRow::default()
///     };
///     HomeRows {
///         rows: vec![row],
///         ..HomeRows::default()
///     }
/// }
///
/// nama_pdk::home_row_source!(Classics, [shared_rows, user_rows]);
/// ```
pub trait HomeRowSource {
    /// The rows every User sees, in the order home shows them.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps the rows the Server stored before.
    fn shared_rows() -> Result<HomeRows, Error> {
        Ok(HomeRows::default())
    }

    /// One User's own rows, in the order home shows them, from that User's Personal settings in
    /// `asked`. Home shows them after the shared rows, leaving out one whose key a shared row
    /// has.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps that User's rows the Server stored before.
    fn user_rows(_asked: UserRowsAsked) -> Result<HomeRows, Error> {
        Ok(HomeRows::default())
    }
}

/// Exports the [`HomeRowSource`] methods a Plugin serves, each under its own name, taking its
/// `nama.plugin.v1` message and returning its `nama.plugin.v1.HomeRows`:
/// `home_row_source!(Classics, [shared_rows, user_rows])`.
///
/// A macro can't see which of the trait's defaults a type overrides, so the list is explicit.
#[macro_export]
macro_rules! home_row_source {
    ($source:ty, [$($method:ident),+ $(,)?]) => {
        $($crate::home_row_source!(@export $source, $method);)+
    };
    (@export $source:ty, shared_rows) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, HomeRowSource, HomeRows, extism_pdk,
                nama_plugin_proto::nama::plugin::v1::SharedRowsAsked, plugin_fn,
            };

            #[plugin_fn]
            pub fn shared_rows(Buffa(_asked): Buffa<SharedRowsAsked>) -> FnResult<Buffa<HomeRows>> {
                Ok(Buffa(<$source as HomeRowSource>::shared_rows()?))
            }
        };
    };
    (@export $source:ty, user_rows) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, HomeRowSource, HomeRows, UserRowsAsked, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn user_rows(Buffa(asked): Buffa<UserRowsAsked>) -> FnResult<Buffa<HomeRows>> {
                Ok(Buffa(<$source as HomeRowSource>::user_rows(asked)?))
            }
        };
    };
}
