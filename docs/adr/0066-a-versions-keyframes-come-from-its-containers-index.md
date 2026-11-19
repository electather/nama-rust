# A Version's keyframes come from its container's index

A Remux needs to know where a Version's keyframes are, and listing them meant reading the whole file, which on a network share takes longer than the 30 s a plan waited, so first plays Transcoded files a Remux could have played (#1214). Probing now reads the keyframes from the container's own index instead: an MKV's Cues, found through its SeekHead, and an MP4's sync-sample table (`stss`, timed by `stts`, `ctts` and `elst`). Both are a few small reads, done by readers in `probing/` rather than a crate, since no crate in the tree reads them and the ones that do parse far more. The whole file is read only for a file without a usable index (TS, AVI without `idx1`, fragmented MP4, an MKV without Cues for its video), only when it's first played, and a plan waits 5 s for it before it Transcodes while the listing finishes for the next play.

Cues may list only some keyframes, while ffmpeg's Remux run still writes a file at every one, so a segment joins the files that start from its first listed keyframe up to the next segment's, by time, not by keyframe number. A file whose Cues are sparse keeps longer segments, and coarser seeking, until it changes; it's never read whole to list them more finely.

## Considered Options

- **Equal-length Remux segments when keyframes aren't listed, as Jellyfin does**: rejected. Their playlist durations don't match where ffmpeg can cut, and it would need a second kind of Remux run. A file without a listed keyframe still Transcodes.
- **List every file's keyframes in the background after a Scan**: rejected. It's a whole-file read per Version from the disk or share playback reads from, for the few files the index doesn't cover.
- **Cut the Remux only at listed keyframes, with ffmpeg's `segment` muxer**: rejected. Joining by time leaves the ffmpeg run and its fMP4 output as they are.

## Consequences

- The change to what Probing reads bumps `PROBE_VERSION`, so the next Scan probes every file again.
