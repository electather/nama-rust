import { Info } from "lucide-react";
import { useEffect, useId, useState } from "react";

import { MoreReasons } from "@/features/player/components/more-reasons";
import { PlaybackTracks } from "@/features/player/components/playback-tracks";
import { Reasons } from "@/features/player/components/reasons";
import { methodName, wordReasons } from "@/features/player/lib/playback-wording";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { Button } from "@/shared/ui/button";

/** The player's "Playback info" button, which opens how its Playback Plan plays, why, and with which Tracks. */
export function PlaybackInfo({ plan }: { plan: PlaybackPlan }) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  const { decided, context } = wordReasons(plan.reasons);
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!open || event.key !== "Escape") {
        return;
      }
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
          className="absolute inset-e-0 top-full mt-2 w-72 rounded-xl border bg-popover p-4 text-sm text-popover-foreground shadow-lg"
        >
          <h2 className="font-medium">{methodName(plan.method)}</h2>
          <Reasons texts={decided} />
          <MoreReasons texts={context} />
          {plan.tracks.length > 0 && <PlaybackTracks tracks={plan.tracks} />}
        </section>
      )}
    </div>
  );
}
