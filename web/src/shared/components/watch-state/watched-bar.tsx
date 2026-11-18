import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { watchedPercent } from "@/shared/lib/watch-progress";
import { Progress } from "@/shared/ui/progress";

/** A bar showing how far in you are, when you've started and not played it. */
export function WatchedBar({ state }: { state: WatchState | undefined }) {
  const percent = watchedPercent(state);
  if (percent === 0) {
    return null;
  }
  return <Progress value={percent} aria-label="Watched" />;
}
