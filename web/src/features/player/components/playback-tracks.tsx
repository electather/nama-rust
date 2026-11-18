import { trackLine } from "@/features/player/lib/track-line";
import type { Track } from "@/gen/nama/v1/browse_pb";

const trackKindNames: Record<NonNullable<Track["kind"]["case"]>, string> = {
  video: "Video",
  audio: "Audio",
  subtitle: "Subtitles",
};

/** The Tracks a plan plays, one line each. */
export function PlaybackTracks({ tracks }: { tracks: Track[] }) {
  return (
    <dl className="mt-4 space-y-1">
      {tracks.map((track) => (
        // The plan has at most one Track of each kind.
        <div key={track.kind.case} className="flex gap-3">
          <dt className="w-16 shrink-0 text-muted-foreground">
            {track.kind.case !== undefined && trackKindNames[track.kind.case]}
          </dt>
          <dd>{trackLine(track)}</dd>
        </div>
      ))}
    </dl>
  );
}
