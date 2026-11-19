# Subtitle Appearance and Offset leave Burn-in and styled ASS alone

Users want to make subtitles bigger or easier to read, and to fix subtitles that are out of sync (#922). A User's **Subtitle Appearance**, a size and a background, is kept on the Server, so it follows them to every Device. A **Subtitle Offset** shifts one subtitle Track's timing. Three decisions shape both:

- **An offset is kept per User, per Version, per subtitle Track.** The usual reason to need one is a sidecar or a downloaded subtitle (ADR 0067) timed for another cut, and each Track can be out of sync by a different amount, so one offset for the whole Version would shift a Track that was already right. It's keyed by how the Version identifies the Track, so a sidecar or a download added later doesn't move an offset onto another Track, and a new download starts at 0. This breaks the per-title rule for a User's data (ADR 0013, ADR 0042) on purpose: an offset is a fact about one file's timing, not about the title, so it goes when its Version goes.
- **Neither applies to Burn-in.** Each would become an input to the Playback Plan, so every nudge would restart the Transcode, which ADR 0033 set out to avoid for subtitle switches. Burn-in is left for formats the Device can't draw, and the player disables those controls for a burned-in Track and says why.
- **A styled ASS Track that the Device draws with libass keeps its author's look**, and takes only the offset. Size and background apply to the WebVTT the Device shows itself, which includes SRT and ASS without fonts (ADR 0033). Rewriting an ASS file's `Style` lines would break signs, karaoke and positioned text.

The background is an **outline** (the default) or a **box**, never bare text, so subtitles keep AA contrast over any frame (PRODUCT.md). There are four named sizes, Small, Default, Large and Largest, relative to each player's own default, rather than a free slider.

## Considered Options

- **An offset per Version**: rejected. An offset for a downloaded subtitle would shift the file's own correctly timed Track as soon as the User switched to it.
- **An offset per Version that resets to 0 when the User picks another Track**: rejected. It's forgotten when the User switches away and back.
- **Size, background and offset as Playback Plan inputs, so they reach Burn-in too**: rejected, as above.
- **Rewriting a styled ASS Track's styles before libass draws it**: rejected, as above. libass in `SubtitlesOctopus` has no way to only scale them.
- **No background, or a soft shadow, as a choice**: rejected. Plain text can't keep AA contrast over a bright frame.

## Consequences

- A styled ASS Track and a burned-in one look the same whatever a User picks; Settings says so.
- Each client styles its own text subtitles from the User's Subtitle Appearance. The web app does it with `::cue`.
