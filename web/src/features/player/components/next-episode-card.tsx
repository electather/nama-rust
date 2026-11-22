import { Button } from "nama-ui/components/button";
import { Progress } from "nama-ui/components/progress";

import { NextEpisodeFacts } from "@/features/player/components/next-episode-facts";
import { PlayNextButton } from "@/features/player/components/play-next-button";
import { COUNTDOWN_SECONDS } from "@/features/player/lib/next-episode";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * The countdown to `episode` near the end of this one, or from the start of its Credits while
 * `inCredits`, with Next Episode and Cancel, which is Watch credits in the Credits.
 */
export function NextEpisodeCard({
  episode,
  secondsLeft,
  inCredits,
  onPlayNext,
  onCancel,
}: {
  episode: Episode;
  secondsLeft: number;
  inCredits: boolean;
  onPlayNext: () => void;
  onCancel: () => void;
}) {
  return (
    // Outside the controls, so it stays while they're hidden, and drops to the edge then. It floats
    // over the video, so it gets a shadow (DESIGN.md, "Elevation").
    <aside
      aria-label="Next Episode"
      className="absolute inset-x-5 bottom-26 z-30 flex flex-col gap-2.5 rounded-xl border bg-popover p-2.5 text-popover-foreground shadow-lg transition-all duration-200 ease-out-expo group-data-controls-hidden:bottom-5 motion-reduce:transition-none sm:inset-s-auto sm:w-100 starting:opacity-0 motion-safe:starting:translate-y-1.5"
    >
      <NextEpisodeFacts episode={episode} secondsLeft={secondsLeft} />
      {/* The timer above says the same in words. */}
      <Progress aria-hidden value={(1 - secondsLeft / COUNTDOWN_SECONDS) * 100} />
      <div className="flex flex-wrap gap-2">
        <PlayNextButton onPlayNext={onPlayNext} />
        <Button variant="ghost" onClick={onCancel}>
          {inCredits ? "Watch credits" : "Cancel"}
        </Button>
      </div>
    </aside>
  );
}
