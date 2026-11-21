import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { isStarted, watchedPercent } from "@/shared/lib/watch-progress";

/**
 * A card's last line: ▶, unless `quiet`, then once started a bar of how far in you are, beside
 * `detail`, such as "33m". Its card's link says all this, so it's hidden from assistive technology.
 */
export function PlayProgress({
  state,
  detail,
  quiet = false,
}: {
  state: WatchState | undefined;
  detail: string | undefined;
  quiet?: boolean;
}) {
  const started = isStarted(state);
  // A sliver even just started, so the bar reads as a bar, as the artboard draws it.
  const played = `${Math.max(8, watchedPercent(state))}%`;
  return (
    <span aria-hidden className="flex items-center gap-2 text-xs">
      {(started || !quiet) && <Play className="size-3 fill-current" />}
      {started && (
        <span
          className="h-1 w-11 flex-none overflow-hidden rounded-full bg-foreground/25 before:block before:h-full before:w-(--played) before:rounded-full before:bg-foreground"
          style={{ "--played": played }}
        />
      )}
      <span className="whitespace-nowrap tabular-nums">{detail}</span>
    </span>
  );
}
