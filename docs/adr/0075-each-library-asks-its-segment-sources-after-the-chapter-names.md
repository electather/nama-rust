# Each Library asks its Segment sources in order, after the Kernel reads the Chapter names

Segments come from several places with different coverage (#1213). Some files name them as Chapters, such as "Opening" or "Previously on", which is exact for that file. Crowdsourced services such as TheIntroDB, SkipDB and AniSkip answer by External ID and duration for what their users have submitted. Audio fingerprinting across a Season, as Jellyfin's Intro Skipper, Emby and Plex do, finds intros no one submitted but costs a decode of every Episode.

A Segment source is a new Extension Point, `nama.plugin.v1.SegmentSourceService`, with a Built-in `SegmentSource` trait mirroring it (ADR 0043). The Kernel sends it a Version's Movie or Episode External IDs, season and episode numbers and duration; it answers with Segments, each a kind (Intro, Recap or Credits), a start and an optional end. Its first Plugin is `plugins/theintrodb`, an Installed Plugin, so an Admin enabling it is what lets External IDs leave the Server.

For each Version, the Kernel first matches its Chapter names against its own list of names for each kind. Then it asks the Library's Segment sources in the Library's order, stored and set as its Recognisers are (ADR 0050). For each kind, the first answer wins, so a kind the Chapters named is never asked for. A source that fails leaves the kinds it would have answered missing, to be asked again.

The Kernel asks in the background after a Version is Probed, never while it plays. A Version still missing a kind is asked again at most every 30 days, since crowdsourced services fill in over time. The work is a query for the Versions that need asking, worked out from what's stored, and one call that asks for a single Version and can run twice. A loop on its own `TaskTracker` joins them today, so a jobs system can take over the loop alone.

A source keeps to its outside service's rate limits; the Kernel doesn't count calls or know any service's limits. But a source can't wait out a limit itself, since each call is fresh and short, so it may answer "try again in N seconds" instead, as a Fulfiller does (`retry_after_seconds`, clamped to 1 to 300). The Kernel then asks that source about no Version until the time is up, while it keeps asking the Library's other sources. The Version that got the answer, and any the source was skipped for, aren't failed: they stay due, and the loop waits for the first wait to end and asks about them again (#1374). As when a source fails, a later source's answer for a kind wins while an earlier one waits.

## Considered Options

- **Kernel-only Chapter names**: rejected. Most files carry no named Chapters.
- **Local audio fingerprinting in the Kernel now**: left out. It needs ffmpeg with Chromaprint, a fingerprint cache and Season-wide comparison, and Installed Plugins can't read media to do it themselves. It comes later if outside services cover too little.
- **One Server-wide order**: rejected for the reasons ADR 0050 gives for Recognisers. An anime service would be asked about every Library.

## Consequences

- The contract adds `SegmentSourceService` 1.0.0; `CONTRACT_VERSION` stays, since only a service is added.
- `SegmentSourceService` 1.1.0 adds `SegmentResults.retry_after_seconds`. A Plugin built before never sets it, and works as before.
- A Version waiting on one source asks the Library's other sources again on the next pass, so a slow source costs the others repeated calls.
- A new Library lists no Segment sources, so only Chapter names find Segments until an Admin lists one.
- TheIntroDB's data terms must be checked before its Plugin ships.
