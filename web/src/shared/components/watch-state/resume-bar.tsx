import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { isStarted, watchedPercent } from "@/shared/lib/watch-progress";
import { Progress } from "@/shared/ui/progress";

/** ▶ and a bar showing how far in you are, unless it's played or you haven't started. */
export function ResumeBar({ state }: { state: WatchState | undefined }) {
  if (!isStarted(state)) {
    return null;
  }
  return (
    <div className="mt-2 flex items-center gap-2 text-foreground">
      <Play aria-hidden className="size-3.5 shrink-0 fill-current" />
      {/* Not `WatchedBar`, which hides under 1%: in Continue Watching, started is started. */}
      <Progress value={watchedPercent(state)} aria-label="Watched" />
    </div>
  );
}
