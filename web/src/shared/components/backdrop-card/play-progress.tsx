import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { useSeenOnce } from "@/shared/hooks/use-seen-once";
import { isStarted, watchedPercent } from "@/shared/lib/watch-progress";

/**
 * The bar, whose fill is as wide as `--played` and grows from its start to that over 900ms the
 * first time the card is in view; under reduced motion it is full at once.
 */
const bar =
  "h-1 w-11 flex-none overflow-hidden rounded-full bg-foreground/25 before:block before:h-full before:w-(--played) before:origin-left before:scale-x-0 before:rounded-full before:bg-foreground before:transition-transform before:duration-900 before:ease-out-expo rtl:before:origin-right data-seen:before:scale-x-100 motion-reduce:before:scale-x-100 motion-reduce:before:transition-none";

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
  const { seen, watch } = useSeenOnce();
  // A sliver even just started, so the bar reads as a bar, as the artboard draws it.
  const played = `${Math.max(8, watchedPercent(state))}%`;
  return (
    <span aria-hidden className="flex items-center gap-2 text-xs">
      {(started || !quiet) && <Play className="size-3 fill-current" />}
      {started && (
        <span
          ref={watch}
          data-seen={seen ? "" : undefined}
          className={bar}
          style={{ "--played": played }}
        />
      )}
      <span className="whitespace-nowrap tabular-nums">{detail}</span>
    </span>
  );
}
