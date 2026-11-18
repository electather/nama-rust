//! A Home row source: gives headed rows of titles for the home page, which the Server draws.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{HomeRow, HomeRowTitle, HomeRows};

/// A Home row source (ADR 0058). Its Manifest's `points` name
/// [`services::HOME_ROW`](crate::services::HOME_ROW), and [`home_row_source!`](crate::home_row_source!)
/// exports it.
///
/// The Server asks ahead of time and stores the answer, so home never waits on the Plugin. It
/// keeps the first 3 rows, the first 200 titles of each and 40 characters of each heading, clamps
/// each time to live to between 15 minutes and 1 week, and shows only the titles on the Server:
///
/// ```
/// use nama_pdk::{
///     HomeRow, HomeRowSource, HomeRowTitle, HomeRows, TitleId, TitleKind, extism_pdk::Error,
/// };
///
/// struct Classics;
///
/// impl HomeRowSource for Classics {
///     fn shared_rows() -> Result<HomeRows, Error> {
///         let alien = HomeRowTitle {
///             kind: TitleKind::TITLE_KIND_MOVIE.into(),
///             id: TitleId {
///                 namespace: "tmdb".to_owned(),
///                 value: "348".to_owned(),
///                 ..TitleId::default()
///             }
///             .into(),
///             ..HomeRowTitle::default()
///         };
///         let row = HomeRow {
///             key: "classics".to_owned(),
///             heading: "Classics".to_owned(),
///             ttl_seconds: 24 * 60 * 60,
///             titles: vec![alien],
///             ..HomeRow::default()
///         };
///         Ok(HomeRows {
///             rows: vec![row],
///             ..HomeRows::default()
///         })
///     }
/// }
///
/// nama_pdk::home_row_source!(Classics);
/// ```
pub trait HomeRowSource {
    /// The rows every User sees, in the order home shows them.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps the rows the Server stored before.
    fn shared_rows() -> Result<HomeRows, Error>;
}

/// Exports a [`HomeRowSource`] as the Plugin's `shared_rows`, taking the
/// `nama.plugin.v1.SharedRowsAsked` the Server sends it and returning its `nama.plugin.v1.HomeRows`.
#[macro_export]
macro_rules! home_row_source {
    ($source:ty) => {
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
}
