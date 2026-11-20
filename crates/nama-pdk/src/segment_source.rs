//! A Segment source: says where a Version's Intro, Recap and Credits are.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    SegmentKind, SegmentResult, SegmentResults, SegmentSearch,
};

/// A Segment source (ADR 0075). Its Manifest's `points` name
/// [`services::SEGMENT_SOURCE`](crate::services::SEGMENT_SOURCE), and
/// [`segment_source!`](crate::segment_source!) exports it.
///
/// The Server asks in the background after probing a Version, only for the kinds it doesn't know
/// yet, and stores the answer itself. The Plugin keeps to its service's rate limits: the Server
/// doesn't count calls.
///
/// ```
/// use nama_pdk::{
///     SegmentKind, SegmentResult, SegmentResults, SegmentSearch, SegmentSource, extism_pdk::Error,
/// };
///
/// struct Fixed;
///
/// impl SegmentSource for Fixed {
///     fn search(_search: SegmentSearch) -> Result<SegmentResults, Error> {
///         let intro = SegmentResult {
///             kind: SegmentKind::SEGMENT_KIND_INTRO.into(),
///             start_ms: 30_000,
///             end_ms: Some(120_000),
///             ..SegmentResult::default()
///         };
///         Ok(SegmentResults {
///             segments: vec![intro],
///             ..SegmentResults::default()
///         })
///     }
/// }
///
/// nama_pdk::segment_source!(Fixed);
/// ```
pub trait SegmentSource {
    /// Where the Segments of the Version `search` describes are, at most one of each kind; empty
    /// if it doesn't know.
    ///
    /// # Errors
    ///
    /// When it couldn't ask, such as past its service's rate limit: the Server asks again later.
    fn search(search: SegmentSearch) -> Result<SegmentResults, Error>;
}

/// Exports a [`SegmentSource`] as the Plugin's `segment_search`, taking the `nama.plugin.v1`
/// message the Server sends it and returning its answer.
#[macro_export]
macro_rules! segment_source {
    ($source:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SegmentResults, SegmentSearch, SegmentSource, extism_pdk,
                plugin_fn,
            };

            #[plugin_fn]
            pub fn segment_search(
                Buffa(search): Buffa<SegmentSearch>,
            ) -> FnResult<Buffa<SegmentResults>> {
                Ok(Buffa(<$source as SegmentSource>::search(search)?))
            }
        };
    };
}
