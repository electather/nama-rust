//! A Similar source: says which titles are like a title.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{SimilarAsked, SimilarTitle, SimilarTitles};

/// A Similar source (ADR 0084). Its Manifest's `points` name
/// [`services::SIMILAR_SOURCE`](crate::services::SIMILAR_SOURCE), and
/// [`similar_source!`](crate::similar_source!) exports it.
///
/// The Server asks when it refreshes a title's Matches and stores the answer, so a page never
/// waits on it, and asks live only for a title not on the Server. It never says who the User is.
///
/// ```
/// use nama_pdk::{SimilarAsked, SimilarSource, SimilarTitle, SimilarTitles, TitleId, extism_pdk::Error};
///
/// struct AlwaysHeat;
///
/// impl SimilarSource for AlwaysHeat {
///     fn similar(_asked: SimilarAsked) -> Result<SimilarTitles, Error> {
///         let heat = SimilarTitle {
///             id: TitleId {
///                 namespace: "tmdb".to_owned(),
///                 value: "949".to_owned(),
///                 ..TitleId::default()
///             }
///             .into(),
///             weight: 0.5,
///             ..SimilarTitle::default()
///         };
///         Ok(SimilarTitles {
///             titles: vec![heat],
///             ..SimilarTitles::default()
///         })
///     }
/// }
///
/// nama_pdk::similar_source!(AlwaysHeat);
/// ```
pub trait SimilarSource {
    /// The titles like the one `asked` names, of its kind, each with a weight from 0.0 to 1.0;
    /// empty if it knows none.
    ///
    /// # Errors
    ///
    /// When it couldn't ask: the Server keeps the answer it stored last.
    fn similar(asked: SimilarAsked) -> Result<SimilarTitles, Error>;
}

/// Exports a [`SimilarSource`] as the Plugin's `similar_titles`, taking the `nama.plugin.v1`
/// message the Server sends it and returning its answer.
#[macro_export]
macro_rules! similar_source {
    ($source:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SimilarAsked, SimilarSource, SimilarTitles, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn similar_titles(
                Buffa(asked): Buffa<SimilarAsked>,
            ) -> FnResult<Buffa<SimilarTitles>> {
                Ok(Buffa(<$source as SimilarSource>::similar(asked)?))
            }
        };
    };
}
