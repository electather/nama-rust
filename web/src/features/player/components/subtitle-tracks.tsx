import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { type PlaybackSubtitle, SubtitleFormat } from "@/gen/nama/v1/playback_pb";

/**
 * The `<video>`'s `<track>`s, one per text subtitle, which the browser fetches only once shown.
 * Those it draws aren't among them.
 */
export function SubtitleTracks({ subtitles }: { subtitles: PlaybackSubtitle[] }) {
  const text = subtitles.filter(({ format }) => format === SubtitleFormat.WEBVTT);
  return text.map(({ track, url }) => (
    <track
      key={url}
      kind="subtitles"
      src={url}
      srcLang={track?.language}
      label={browserMenuLabel(track)}
    />
  ));
}

/** What the browser's own subtitle menu, such as in an iPhone's full screen, calls it. */
function browserMenuLabel(track: Track | undefined) {
  const { name, details } = trackLabel(track);
  return details === "" ? name : `${name} (${details})`;
}
