//! A Search source: adds titles to a User's search.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    SearchSourceQuery, SearchSourceResults, SearchSourceTitle,
};

/// A Search source (ADR 0083). Its Manifest's `points` name
/// [`services::SEARCH_SOURCE`](crate::services::SEARCH_SOURCE), and
/// [`search_source!`](crate::search_source!) exports it.
///
/// The Server asks it for every search that isn't blank, and never says who is searching. It
/// shows a title only by an id it knows or an enabled title source keeps; a poster path is fetched
/// through this Plugin's own `title_image` export.
///
/// ```
/// use nama_pdk::{
///     SearchSource, SearchSourceQuery, SearchSourceResults, SearchSourceTitle, TitleId,
///     TitleKind, extism_pdk::Error,
/// };
///
/// struct Dreams;
///
/// impl SearchSource for Dreams {
///     fn search(_query: SearchSourceQuery) -> Result<SearchSourceResults, Error> {
///         let inception = SearchSourceTitle {
///             kind: TitleKind::TITLE_KIND_MOVIE.into(),
///             ids: vec![TitleId {
///                 namespace: "tmdb".to_owned(),
///                 value: "27205".to_owned(),
///                 ..TitleId::default()
///             }],
///             title: "Inception".to_owned(),
///             year: Some(2010),
///             ..SearchSourceTitle::default()
///         };
///         Ok(SearchSourceResults {
///             titles: vec![inception],
///             ..SearchSourceResults::default()
///         })
///     }
/// }
///
/// nama_pdk::search_source!(Dreams);
/// ```
pub trait SearchSource {
    /// The titles that best answer `query`, best first; empty if none fit.
    ///
    /// # Errors
    ///
    /// When it couldn't answer: the search names the Plugin as failed and goes on without it.
    fn search(query: SearchSourceQuery) -> Result<SearchSourceResults, Error>;
}

/// Exports a [`SearchSource`] as the Plugin's `search_titles`, taking the `nama.plugin.v1`
/// message the Server sends it and returning its answer.
#[macro_export]
macro_rules! search_source {
    ($source:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SearchSource, SearchSourceQuery, SearchSourceResults, extism_pdk,
                plugin_fn,
            };

            #[plugin_fn]
            pub fn search_titles(
                Buffa(query): Buffa<SearchSourceQuery>,
            ) -> FnResult<Buffa<SearchSourceResults>> {
                Ok(Buffa(<$source as SearchSource>::search(query)?))
            }
        };
    };
}
