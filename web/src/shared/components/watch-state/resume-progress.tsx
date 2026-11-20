import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { watchedPercent } from "@/shared/lib/watch-progress";

declare module "react" {
  interface CSSProperties {
    /** How far into a title you are, as a bar draws it. */
    "--played"?: string;
  }
}

/**
 * Inside a Resume pill, in its ink: a bar of how far in you are, and what's `left`. The pill's
 * name says both, so this is hidden from assistive technology.
 */
export function ResumeProgress({ state, left }: { state: WatchState; left: string | undefined }) {
  return (
    <span aria-hidden className="flex items-center gap-2.5 tabular-nums">
      <span
        className="h-1.5 w-16 overflow-hidden rounded-full bg-primary-foreground/20 before:block before:h-full before:w-(--played) before:min-w-1.5 before:rounded-full before:bg-primary-foreground"
        style={{ "--played": `${watchedPercent(state)}%` }}
      />
      {left}
    </span>
  );
}
