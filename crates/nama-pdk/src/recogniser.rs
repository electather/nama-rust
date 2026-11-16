//! A Recogniser: proposes what a file is or which title an Item names, and the Kernel decides.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    FileRecognised, FileToRecognise, RecognisedEpisodes, RecognisedMovie, TitleRecognised,
    TitleToRecognise, file_recognised::Recognised,
};

/// A Recogniser: proposes what a file is or which title an Item names, and the Server decides
/// (ADR 0050). Its Manifest's `points` name [`services::RECOGNISER`](crate::services::RECOGNISER).
/// Both methods are optional, and only those [`recogniser!`](crate::recogniser!) lists are
/// exported; the Server asks nothing of a Recogniser that lacks the export.
///
/// An answer left empty means it can't tell, and the Server asks the next Recogniser in the
/// Library's order; a failure leaves the Item or file undecided until the next Scan:
///
/// ```
/// use nama_pdk::{
///     FileRecognised, FileToRecognise, RecognisedEpisodes, Recogniser, extism_pdk::Error,
/// };
///
/// struct Anime;
///
/// impl Recogniser for Anime {
///     // `Show - 137.mkv`: Episode 137, numbered from the start of the Show.
///     fn recognise_file(file: FileToRecognise) -> Result<FileRecognised, Error> {
///         let name = file.path.rsplit('/').next().unwrap_or_default();
///         let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
///         let Some((show, number)) = stem.rsplit_once(" - ") else {
///             return Ok(FileRecognised::default());
///         };
///         let Ok(episode) = number.parse() else {
///             return Ok(FileRecognised::default());
///         };
///         let episodes = RecognisedEpisodes {
///             show_title: show.to_owned(),
///             season: 1,
///             episodes: vec![episode],
///             ..RecognisedEpisodes::default()
///         };
///         Ok(FileRecognised {
///             recognised: Some(episodes.into()),
///             ..FileRecognised::default()
///         })
///     }
/// }
///
/// nama_pdk::recogniser!(Anime, [recognise_file]);
/// ```
pub trait Recogniser {
    /// The ids of the title an Item names, as `title` describes it with what the Library's title
    /// sources found for it; none if it can't tell. The Server takes them only in a namespace one
    /// of the Library's title sources serves.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which leaves the Item undecided until the next Scan.
    fn recognise_title(_title: TitleToRecognise) -> Result<TitleRecognised, Error> {
        Ok(TitleRecognised::default())
    }

    /// What `file` is, a file the Server's name parser made no Item from: a Movie, or Episodes of
    /// a Show, which the Server then Matches as usual; unset if it can't tell.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which leaves the file unparsed until the next Scan.
    fn recognise_file(_file: FileToRecognise) -> Result<FileRecognised, Error> {
        Ok(FileRecognised::default())
    }
}

/// Exports the [`Recogniser`] methods a Plugin serves, each under its own name, taking and
/// returning their `nama.plugin.v1` messages: `recogniser!(Anime, [recognise_file])`.
///
/// The Server asks a Recogniser only for the exports it has, so only the methods listed are
/// exported. A macro can't see which of the trait's defaults a type overrides, so the list is
/// explicit.
#[macro_export]
macro_rules! recogniser {
    ($recogniser:ty, [$($method:ident),+ $(,)?]) => {
        $($crate::recogniser!(@export $recogniser, $method);)+
    };
    (@export $recogniser:ty, recognise_title) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, Recogniser, TitleRecognised, TitleToRecognise, extism_pdk,
                plugin_fn,
            };

            #[plugin_fn]
            pub fn recognise_title(
                Buffa(title): Buffa<TitleToRecognise>,
            ) -> FnResult<Buffa<TitleRecognised>> {
                Ok(Buffa(<$recogniser as Recogniser>::recognise_title(title)?))
            }
        };
    };
    (@export $recogniser:ty, recognise_file) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FileRecognised, FileToRecognise, FnResult, Recogniser, extism_pdk,
                plugin_fn,
            };

            #[plugin_fn]
            pub fn recognise_file(
                Buffa(file): Buffa<FileToRecognise>,
            ) -> FnResult<Buffa<FileRecognised>> {
                Ok(Buffa(<$recogniser as Recogniser>::recognise_file(file)?))
            }
        };
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        FileRecognised, FileToRecognise, Recogniser, TitleRecognised, TitleToRecognise,
        extism_pdk::Error,
    };

    struct Files;

    impl Recogniser for Files {
        fn recognise_file(_file: FileToRecognise) -> Result<FileRecognised, Error> {
            Ok(FileRecognised::default())
        }
    }

    struct Titles;

    impl Recogniser for Titles {
        fn recognise_title(_title: TitleToRecognise) -> Result<TitleRecognised, Error> {
            Ok(TitleRecognised::default())
        }
    }

    // Each export is a `no_mangle` symbol, so these two link only if `recogniser!` exports
    // nothing but the methods it lists: an unlisted `recognise_title` would be defined twice.
    crate::recogniser!(Files, [recognise_file]);
    crate::recogniser!(Titles, [recognise_title]);

    #[test]
    fn a_method_left_out_cant_tell() {
        let title = Files::recognise_title(TitleToRecognise::default()).unwrap();
        let file = Titles::recognise_file(FileToRecognise::default()).unwrap();

        assert_eq!(title, TitleRecognised::default());
        assert_eq!(file, FileRecognised::default());
    }
}
