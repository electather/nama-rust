//! A subtitle source: searches an outside service for a Version's subtitles and downloads one.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    SubtitleDownload, SubtitleFile, SubtitleResult, SubtitleResults, SubtitleSearch,
};

/// A subtitle source (ADR 0067). Its Manifest's `points` name
/// [`services::SUBTITLE_SOURCE`](crate::services::SUBTITLE_SOURCE), and
/// [`subtitle_source!`](crate::subtitle_source!) exports it.
///
/// The Server asks only while a User with Download subtitles watches, and stores the download
/// itself. The Plugin keeps to its service's rate limits: the Server doesn't count calls.
///
/// ```
/// use nama_pdk::{
///     SubtitleDownload, SubtitleFile, SubtitleResult, SubtitleResults, SubtitleSearch,
///     SubtitleSource, extism_pdk::Error,
/// };
///
/// struct Fixed;
///
/// impl SubtitleSource for Fixed {
///     fn search(_search: SubtitleSearch) -> Result<SubtitleResults, Error> {
///         let result = SubtitleResult {
///             id: "1".to_owned(),
///             language: "en".to_owned(),
///             name: "Alien.1979.1080p.BluRay".to_owned(),
///             ..SubtitleResult::default()
///         };
///         Ok(SubtitleResults {
///             results: vec![result],
///             ..SubtitleResults::default()
///         })
///     }
///
///     fn download(_download: SubtitleDownload) -> Result<SubtitleFile, Error> {
///         Ok(SubtitleFile {
///             data: b"1\n00:00:01,000 --> 00:00:02,000\nHello\n".to_vec(),
///             format: "srt".to_owned(),
///             ..SubtitleFile::default()
///         })
///     }
/// }
///
/// nama_pdk::subtitle_source!(Fixed);
/// ```
pub trait SubtitleSource {
    /// The source's subtitles for the Version `search` describes, best first; empty if none fit.
    ///
    /// # Errors
    ///
    /// When it couldn't search, such as past its service's rate limit.
    fn search(search: SubtitleSearch) -> Result<SubtitleResults, Error>;

    /// The file of the subtitle a search answered with `download`'s id, at most
    /// [`BLOB_MAX_BYTES`](crate::BLOB_MAX_BYTES).
    ///
    /// # Errors
    ///
    /// When it couldn't download it, such as past its service's daily quota.
    fn download(download: SubtitleDownload) -> Result<SubtitleFile, Error>;
}

/// Exports a [`SubtitleSource`] as the Plugin's `subtitle_search` and `subtitle_download`, taking
/// the `nama.plugin.v1` messages the Server sends them and returning theirs.
#[macro_export]
macro_rules! subtitle_source {
    ($source:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SubtitleDownload, SubtitleFile, SubtitleResults, SubtitleSearch,
                SubtitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn subtitle_search(
                Buffa(search): Buffa<SubtitleSearch>,
            ) -> FnResult<Buffa<SubtitleResults>> {
                Ok(Buffa(<$source as SubtitleSource>::search(search)?))
            }

            #[plugin_fn]
            pub fn subtitle_download(
                Buffa(download): Buffa<SubtitleDownload>,
            ) -> FnResult<Buffa<SubtitleFile>> {
                Ok(Buffa(<$source as SubtitleSource>::download(download)?))
            }
        };
    };
}
