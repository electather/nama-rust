//! Text subtitles, served as WebVTT for a player's `<track>`.
//!
//! Where a subtitle comes from is kept apart from how it's served. [`Embedded`] is the one source
//! so far, a text subtitle Track inside a Version's file. [`Cache`] serves any source's WebVTT: it
//! has the source make it the first time it's asked for, keeps it under `subtitles/` in the data
//! directory, and answers from there after. Subtitle files next to a video would be another source
//! served by the same [`Cache`].
//!
//! Clients fetch them at `/media/versions/<id>/subtitles/<track>` (ADR 0007: media bytes are not
//! RPC), which, like the Version itself, is served to anyone with a session.

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Router,
    extract::{Path as UrlPath, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{
    artwork::write_atomically,
    contents::Contents,
    ffmpeg::{Ffmpeg, SubtitleError},
    tracks::{TrackKind, Tracks},
};

/// Where clients fetch the Version `version`'s Track `track`, by its place in the Version's
/// Tracks, as WebVTT.
pub(super) fn url(version: i64, track: usize) -> String {
    format!("/media/versions/{version}/subtitles/{track}")
}

/// What serving a subtitle needs: what finds its source and what reads it.
#[derive(Debug, Clone)]
pub(super) struct Subtitles {
    pub(super) contents: Contents,
    pub(super) tracks: Tracks,
    pub(super) ffmpeg: Arc<Ffmpeg>,
    pub(super) cache: Cache,
}

/// Serves `GET` [`url`]s, 404 unless the Track is a text subtitle of a Version. Whether the caller
/// has a session is up to the layers the router is put behind.
pub(super) fn router(subtitles: Subtitles) -> Router {
    Router::new()
        .route("/media/versions/{version}/subtitles/{track}", get(serve))
        .with_state(subtitles)
}

async fn serve(
    State(subtitles): State<Subtitles>,
    UrlPath((version, track)): UrlPath<(i64, usize)>,
) -> Response {
    let source = match Embedded::find(&subtitles.contents, &subtitles.tracks, version, track).await
    {
        Ok(Some(source)) => source,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s Track {track}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let webvtt = subtitles
        .cache
        .webvtt(&source.cache_key(), source.webvtt(&subtitles.ffmpeg))
        .await;
    match webvtt {
        Ok(webvtt) => ([(header::CONTENT_TYPE, "text/vtt; charset=utf-8")], webvtt).into_response(),
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s Track {track}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Why a subtitle couldn't be served.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("subtitle cache error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Read(#[from] SubtitleError),
}

/// A text subtitle Track inside a Version's file.
struct Embedded {
    version: i64,
    /// Its place in the Version's Tracks.
    track: usize,
    path: String,
    /// Its place among the file's subtitle Tracks, which is how ffmpeg picks it out.
    subtitle: usize,
}

impl Embedded {
    /// The Version `version`'s Track `track`, `None` if there's no such Version or the Track isn't
    /// a text subtitle.
    async fn find(
        contents: &Contents,
        tracks: &Tracks,
        version: i64,
        track: usize,
    ) -> Result<Option<Self>, sqlx::Error> {
        let Some((path, _)) = contents.version_file(version).await? else {
            return Ok(None);
        };
        let Some(probe) = tracks.of(&[version]).await?.remove(&version) else {
            return Ok(None);
        };
        let is_text = (probe.tracks.get(track)).is_some_and(|found| found.kind.is_text_subtitle());
        if !is_text {
            return Ok(None);
        }
        // Probing keeps every subtitle stream, so ffmpeg's subtitle streams are these, in order.
        let subtitle = (probe.tracks.iter().take(track))
            .filter(|earlier| matches!(earlier.kind, TrackKind::Subtitle { .. }))
            .count();
        Ok(Some(Self {
            version,
            track,
            path,
            subtitle,
        }))
    }

    fn cache_key(&self) -> PathBuf {
        Path::new("versions")
            .join(self.version.to_string())
            .join(format!("{}.vtt", self.track))
    }

    async fn webvtt(&self, ffmpeg: &Ffmpeg) -> Result<Vec<u8>, Error> {
        Ok(ffmpeg.webvtt(&self.path, self.subtitle).await?)
    }
}

/// WebVTT on disk, made by its source the first time it's asked for.
#[derive(Debug, Clone)]
pub(super) struct Cache {
    dir: PathBuf,
}

impl Cache {
    /// The cache in `data_dir`.
    pub(super) fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("subtitles"),
        }
    }

    /// The WebVTT cached at `key`, or else what `make` makes, which is cached there from then on.
    async fn webvtt<E: From<io::Error>>(
        &self,
        key: &Path,
        make: impl Future<Output = Result<Vec<u8>, E>>,
    ) -> Result<Vec<u8>, E> {
        let path = self.dir.join(key);
        match tokio::fs::read(&path).await {
            Ok(webvtt) => return Ok(webvtt),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        let webvtt = make.await?;
        write_atomically(&path, &webvtt).await?;
        Ok(webvtt)
    }
}

#[cfg(test)]
mod tests {
    use std::{io, path::Path};

    use super::Cache;

    #[tokio::test]
    async fn a_subtitle_is_made_by_its_source_once_then_served_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path());
        let key = Path::new("versions/7/2.vtt");

        let made = cache.webvtt(key, async { Ok::<_, io::Error>(b"WEBVTT\n".to_vec()) });
        let made = made.await.unwrap();
        let again = cache.webvtt(key, async { Err(io::Error::other("made again")) });
        let again = again.await.unwrap();

        assert_eq!(made, b"WEBVTT\n");
        assert_eq!(again, b"WEBVTT\n");
    }
}
