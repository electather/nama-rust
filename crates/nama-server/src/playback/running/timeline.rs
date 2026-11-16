use std::{fmt::Write as _, ops::Range};

use super::INIT;
use crate::ffmpeg::Keyframe;

/// How long segments aim to be, in milliseconds.
pub(super) const SEGMENT_MS: i64 = 6000;

/// Where a Version's keyframes fall, and the segments its playlist lists, made of them.
pub(super) struct Timeline {
    /// When each keyframe falls, in milliseconds, in order.
    pub(super) keyframes: Vec<i64>,
    pub(super) segments: Vec<Segment>,
}

/// One segment a playlist lists.
pub(super) struct Segment {
    /// The keyframe it starts at, by number.
    first_keyframe: usize,
    /// How long it lasts, in milliseconds.
    pub(super) ms: i64,
}

impl Timeline {
    /// A video with `keyframes`, in milliseconds, cut at the first keyframe at or after each
    /// multiple of [`SEGMENT_MS`] past the last cut's multiple. No cut falls at or after
    /// `duration_ms`, where the last segment ends, so none lasts 0 ms.
    pub(super) fn new(keyframes: Vec<i64>, duration_ms: i64) -> Self {
        let mut segments = vec![];
        let (mut first_keyframe, mut start, mut cut_after) = (0, 0, SEGMENT_MS);
        for (n, &keyframe) in keyframes.iter().enumerate() {
            if keyframe >= cut_after && keyframe < duration_ms {
                let ms = keyframe - start;
                segments.push(Segment { first_keyframe, ms });
                (first_keyframe, start) = (n, keyframe);
                cut_after += SEGMENT_MS;
            }
        }
        let ms = duration_ms - start;
        segments.push(Segment { first_keyframe, ms });
        Self {
            keyframes,
            segments,
        }
    }

    /// A Transcode of a video `duration_ms` long, with a keyframe forced at each multiple of
    /// [`SEGMENT_MS`] before its end, so each is a segment's first.
    pub(super) fn transcoded(duration_ms: i64) -> Self {
        let segments = (duration_ms + SEGMENT_MS - 1) / SEGMENT_MS;
        Self::new((0..segments).map(|n| n * SEGMENT_MS).collect(), duration_ms)
    }

    /// The keyframes whose files segment `n` joins, by number.
    pub(super) fn keyframes_of(&self, n: usize) -> Range<usize> {
        let first = self
            .segments
            .get(n)
            .map_or(0, |segment| segment.first_keyframe);
        let next = self.segments.get(n + 1);
        first..next.map_or(self.keyframes.len(), |next| next.first_keyframe)
    }

    /// When keyframe `n` falls, in milliseconds, or the end of time for one past the last.
    pub(super) fn keyframe_ms(&self, n: usize) -> i64 {
        self.keyframes.get(n).copied().unwrap_or(i64::MAX)
    }

    /// When segment `n` starts, in milliseconds.
    pub(super) fn segment_start_ms(&self, n: usize) -> i64 {
        self.keyframe_ms(self.keyframes_of(n).start)
    }

    /// The segment playing at `ms`, by number: the last one starting at or before it, or the
    /// first for a position before any.
    pub(super) fn segment_at(&self, ms: i64) -> usize {
        let starting_by = (self.segments)
            .partition_point(|segment| self.keyframe_ms(segment.first_keyframe) <= ms);
        starting_by.saturating_sub(1)
    }

    /// Keyframe `n`, for ffmpeg to start at.
    pub(super) fn keyframe(&self, n: usize) -> Keyframe {
        let previous_ms = n
            .checked_sub(1)
            .and_then(|previous| self.keyframes.get(previous));
        Keyframe {
            n,
            ms: self.keyframes.get(n).copied().unwrap_or_default(),
            previous_ms: previous_ms.copied(),
        }
    }

    /// The HLS playlist listing segments `0.m4s`, `1.m4s` and so on.
    pub(super) fn playlist(&self) -> String {
        let longest = self.segments.iter().map(|segment| segment.ms).max();
        let mut playlist = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{INIT}\"\n",
            // Whole seconds, rounded up, as HLS asks.
            (longest.unwrap_or_default() + 999) / 1000,
        );
        for (n, Segment { ms, .. }) in self.segments.iter().enumerate() {
            // Writing to a `String` never fails.
            let _ = write!(
                playlist,
                "#EXTINF:{}.{:03},\n{n}.m4s\n",
                ms / 1000,
                ms % 1000
            );
        }
        playlist.push_str("#EXT-X-ENDLIST\n");
        playlist
    }
}

/// The keyframe whose file is named `name`, if it's one.
pub(super) fn keyframe_number(name: &str) -> Option<usize> {
    name.strip_suffix(".m4s")?.parse().ok()
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
