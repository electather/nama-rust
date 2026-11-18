import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { WatchedBar } from "@/shared/components/watch-state/watched-bar";

/** Beside a Resume pill: a bar showing how far in you are, and `detail`, such as how much is left. */
export function ResumeDetail({ state, detail }: { state: WatchState; detail: string | undefined }) {
  return (
    <div className="flex items-center gap-3 text-sm text-muted-foreground">
      <div className="w-20">
        <WatchedBar state={state} />
      </div>
      {detail}
    </div>
  );
}
