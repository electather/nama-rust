//! Downloaded subtitles: searching the subtitle sources for a Version's subtitles, keeping one a
//! User downloads in the blob store, and deleting a Version's once it's gone (ADR 0067).
//!
//! A download plays as a sidecar does, from a file on local disk ([`ImageStore::write_on_disk`]),
//! and [`Sidecars`] lists it after the Version's sidecars.

use std::{io, sync::Arc};

use futures_util::future::join_all;
use nama_plugin_api::{
    PluginId,
    contract::{
        SubtitleDownload, SubtitleFile, SubtitleResult, SubtitleSearch, TitleId, TitleKind,
    },
    subtitle_source::SubtitleSource,
};

use super::{
    Sidecar, Sidecars, Track,
    sidecars::{codec_of, names_language, subtitle},
};
use crate::{
    db::{self, Database},
    images::{DOWNLOADED_SUBTITLES_DIR as DIR, ImageStore, MAX_BYTES, version_of},
    library::{Contents, ItemKind},
    plugins::{Answer, ExtensionPoints},
};

/// Why a subtitle couldn't be searched for or downloaded.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("there is no such Version")]
    NoSuchVersion,
    #[error("{0:?} isn't a language tag, such as `en` or `pt-BR`")]
    UnknownLanguage(String),
    /// No source could answer, or the one asked gave no subtitle: why, in plain words.
    #[error("{0}")]
    NoSource(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("couldn't keep the subtitle: {0}")]
    Store(#[from] io::Error),
}

/// Searches the subtitle sources and keeps the subtitles Users download from them.
#[derive(Debug, Clone)]
pub(crate) struct SubtitleDownloads {
    contents: Contents,
    sidecars: Sidecars,
    points: ExtensionPoints,
    images: ImageStore,
}

impl SubtitleDownloads {
    pub(crate) fn new(db: &Arc<Database>, points: ExtensionPoints, images: ImageStore) -> Self {
        Self {
            contents: Contents::new(Arc::clone(db)),
            sidecars: Sidecars::new(Arc::clone(db)),
            points,
            images,
        }
    }

    /// Whether a subtitle source is enabled, so [`Self::search`] has one to ask.
    pub(crate) fn has_source(&self) -> bool {
        !self.points.providers::<dyn SubtitleSource>().is_empty()
    }

    /// What every enabled subtitle source finds for the Version `version` in `languages`, BCP 47
    /// tags, each found by the Plugin whose source it is, each source's best first. A source that
    /// fails is left out.
    ///
    /// # Errors
    ///
    /// [`Error::NoSource`] if none is enabled or every one failed, and as the variants say.
    pub(crate) async fn search(
        &self,
        version: i64,
        languages: Vec<String>,
    ) -> Result<Vec<(PluginId, SubtitleResult)>, Error> {
        if let Some(unknown) = languages.iter().find(|tag| !names_language(tag)) {
            return Err(Error::UnknownLanguage(unknown.clone()));
        }
        let search = self.search_for(version, languages).await?;
        let sources = self.points.providers::<dyn SubtitleSource>();
        if sources.is_empty() {
            return Err(Error::NoSource("no subtitle source is enabled".to_owned()));
        }
        let asked = sources.iter().map(|plugin| {
            let search = &search;
            self.points
                .ask(plugin, move |source: Arc<dyn SubtitleSource>| async move {
                    source.search(search).await
                })
        });
        let (mut found, mut failures, mut answered) = (Vec::new(), Vec::new(), false);
        for (source, answer) in sources.iter().zip(join_all(asked).await) {
            match answer {
                Some(Answer {
                    plugin,
                    result: Ok(results),
                }) => {
                    answered = true;
                    found.extend(results.results.into_iter().map(|r| (plugin.clone(), r)));
                }
                Some(Answer {
                    result: Err(failure),
                    ..
                }) => failures.push(failure.message),
                // A source that panicked, and is disabled now, failed too.
                None => failures.push(format!("{source} stopped working and was disabled")),
            }
        }
        if !answered {
            return Err(Error::NoSource(failures.join("; ")));
        }
        Ok(found)
    }

    /// What a subtitle source is asked about the Version `version`.
    async fn search_for(
        &self,
        version: i64,
        languages: Vec<String>,
    ) -> Result<SubtitleSearch, Error> {
        let item = (self.contents.first_item(version).await?).ok_or(Error::NoSuchVersion)?;
        let file_name = item.path.rsplit(['/', '\\']).next().unwrap_or_default();
        let (season, episode) = item.episode.unzip();
        Ok(SubtitleSearch {
            kind: match item.kind {
                ItemKind::Movie => TitleKind::TITLE_KIND_MOVIE,
                ItemKind::Episode => TitleKind::TITLE_KIND_SHOW,
            }
            .into(),
            ids: (item.ids.iter())
                .map(|id| TitleId {
                    namespace: id.namespace().to_owned(),
                    value: id.value().to_owned(),
                    ..TitleId::default()
                })
                .collect(),
            season: season.and_then(|number| u32::try_from(number).ok()),
            episode: episode.and_then(|number| u32::try_from(number).ok()),
            file_name: file_name.to_owned(),
            // ponytail: no OpenSubtitles hash, which reads the file's ends; add it once a source
            // finds too little by ids and name.
            hash: None,
            languages,
            ..SubtitleSearch::default()
        })
    }

    /// Downloads `found` from the Plugin `plugin`'s subtitle source and keeps it as the Version
    /// `version`'s last Track, unless it's kept already.
    ///
    /// # Errors
    ///
    /// [`Error::NoSource`] if the Plugin isn't an enabled subtitle source, or its download fails,
    /// isn't a subtitle or is over 20 MiB, and as the other variants say.
    pub(crate) async fn download(
        &self,
        version: i64,
        plugin: &PluginId,
        found: SubtitleResult,
    ) -> Result<(), Error> {
        if !names_language(&found.language) {
            return Err(Error::UnknownLanguage(found.language));
        }
        if self.contents.version_file(version).await?.is_none() {
            return Err(Error::NoSuchVersion);
        }
        let file = self.fetch(plugin, found.id).await?;
        let (codec, extension) = codec_of(&file.format)
            .map(|codec| (codec, file.format.to_ascii_lowercase()))
            .ok_or_else(|| Error::NoSource(format!("{:?} isn't a subtitle format", file.format)))?;
        if file.data.len() > MAX_BYTES {
            return Err(Error::NoSource("the subtitle is over 20 MiB".to_owned()));
        }
        let name = format!("{}.{extension}", version_of(&file.data));
        let key = [DIR, &version.to_string(), &name];
        let path = self.images.write_on_disk(&key, &file.data).await?;
        let path = (path.into_os_string().into_string())
            .map_err(|_not_utf8| io::Error::other("the data directory's path isn't UTF-8"))?;
        let sidecar = Sidecar {
            path,
            size: i64::try_from(file.data.len()).unwrap_or(i64::MAX),
            modified_ns: db::now_ms().saturating_mul(1_000_000),
        };
        let track = Track {
            kind: subtitle(codec, None),
            codec: codec.to_owned(),
            language: Some(found.language),
            title: Some(found.name),
            default: false,
            forced: false,
            sdh: found.sdh,
        };
        Ok(self.sidecars.keep_download(version, sidecar, track).await?)
    }

    /// The file the Plugin `plugin`'s subtitle source downloads as `id`.
    async fn fetch(&self, plugin: &PluginId, id: String) -> Result<SubtitleFile, Error> {
        let asked = SubtitleDownload {
            id,
            ..SubtitleDownload::default()
        };
        let answer = (self.points)
            .ask(plugin, |source: Arc<dyn SubtitleSource>| {
                let asked = &asked;
                async move { source.download(asked).await }
            })
            .await
            .ok_or_else(|| Error::NoSource(format!("{plugin} isn't an enabled subtitle source")))?;
        answer
            .result
            .map_err(|failure| Error::NoSource(failure.message))
    }
}
