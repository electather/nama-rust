import { Info } from "lucide-react";
import { useEffect, useId, useState } from "react";

import { Button } from "@/components/ui/button";
import type { Track } from "@/gen/nama/v1/browse_pb";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { trackLine } from "@/lib/browse";
import { methodNames, wordReasons } from "@/lib/playback-wording";

const trackKindNames: Record<NonNullable<Track["kind"]["case"]>, string> = {
  video: "Video",
  audio: "Audio",
  subtitle: "Subtitles",
};

/** The player's "Playback info" button, which opens how its Playback Plan plays, why, and with which Tracks. */
export function PlaybackInfo({ plan }: { plan: PlaybackPlan }) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  const { decided, context } = wordReasons(plan.reasons);
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!open || event.key !== "Escape") return;
      // Esc otherwise leaves the player (`useShortcuts`), wherever focus is, so this listens on the
      // document too, and first.
      event.preventDefault();
      setOpen(false);
    }
    document.addEventListener("keydown", onKeyDown, { capture: true });
    return () => {
      document.removeEventListener("keydown", onKeyDown, { capture: true });
    };
  }, [open]);
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
      >
        <Info aria-hidden />
      </Button>
      {open && (
        <section
          id={panelId}
          aria-label="Playback info"
          className="absolute end-0 top-full mt-2 w-72 rounded-xl border bg-popover p-4 text-sm text-popover-foreground shadow-lg"
        >
          <h2 className="font-medium">{methodNames.get(plan.method)}</h2>
          <Reasons texts={decided} />
          <MoreReasons texts={context} />
          {plan.tracks.length > 0 && <Tracks tracks={plan.tracks} />}
        </section>
      )}
    </div>
  );
}

function Reasons({ texts }: { texts: string[] }) {
  return (
    <ul className="mt-1 space-y-1 text-muted-foreground">
      {texts.map((text) => (
        <li key={text}>{text}</li>
      ))}
    </ul>
  );
}

/** The reasons that are only context, folded away until opened. */
function MoreReasons({ texts }: { texts: string[] }) {
  if (texts.length === 0) return null;
  return (
    <details className="mt-2">
      <summary className="cursor-pointer">More</summary>
      <Reasons texts={texts} />
    </details>
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
