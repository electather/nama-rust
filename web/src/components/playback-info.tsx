import { Info } from "lucide-react";
import { useId, useState } from "react";

import { Button } from "@/components/ui/button";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PlaybackMethod, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { trackLine } from "@/lib/browse";

const methodNames: Record<PlaybackMethod, string> = {
  [PlaybackMethod.UNSPECIFIED]: "Can’t play here",
  [PlaybackMethod.DIRECT_PLAY]: "Direct Play",
  [PlaybackMethod.REMUX]: "Remux",
  [PlaybackMethod.TRANSCODE]: "Transcode",
};

const trackKindNames: Record<NonNullable<Track["kind"]["case"]>, string> = {
  video: "Video",
  audio: "Audio",
  subtitle: "Subtitles",
};

/** The player's "Playback info" button, which opens how its Playback Plan plays, why, and with which Tracks. */
export function PlaybackInfo({ plan }: { plan: PlaybackPlan }) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  return (
    <div className="relative">
      <Button
        variant="overlay"
        size="icon"
        aria-label="Playback info"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => {
          setOpen(!open);
        }}
        onKeyDown={(event) => {
          if (event.key !== "Escape" || !open) return;
          // Esc otherwise leaves the player (`useShortcuts`, listening on the document).
          event.stopPropagation();
          setOpen(false);
        }}
      >
        <Info aria-hidden />
      </Button>
      {open && (
        <section
          id={panelId}
          aria-label="Playback info"
          className="absolute end-0 top-full mt-2 w-72 rounded-xl border bg-popover p-4 text-sm text-popover-foreground shadow-lg"
        >
          <h2 className="font-medium">{methodNames[plan.method]}</h2>
          <ul className="mt-1 text-muted-foreground">
            {plan.reasons.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
          {plan.tracks.length > 0 && <Tracks tracks={plan.tracks} />}
        </section>
      )}
    </div>
  );
}

function Tracks({ tracks }: { tracks: Track[] }) {
  return (
    <dl className="mt-4 space-y-1">
      {tracks.map((track) => (
        // The plan has at most one Track of each kind.
        <div key={track.kind.case} className="flex gap-3">
          <dt className="w-16 shrink-0 text-muted-foreground">
            {track.kind.case && trackKindNames[track.kind.case]}
          </dt>
          <dd>{trackLine(track)}</dd>
        </div>
      ))}
    </dl>
  );
}
