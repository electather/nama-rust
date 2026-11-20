import { Link, type LinkOptions } from "@tanstack/react-router";
import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { ResumeProgress } from "@/shared/components/watch-state/resume-progress";
import { remainingOf } from "@/shared/lib/title-facts";
import { isStarted } from "@/shared/lib/watch-progress";
import { buttonVariants } from "@/shared/ui/button";

/**
 * The pill that plays `link`: "Play", or once started, how far in you are and what's left, such
 * as ▶ ━━── 33m, which it's named for, "Resume, 33m left".
 */
export function PlayButton({ link, state }: { link: LinkOptions; state: WatchState | undefined }) {
  const started = isStarted(state);
  const left = remainingOf(state);
  return (
    <Link
      {...link}
      aria-label={started ? resumeName(left) : undefined}
      className={buttonVariants({ size: "pill" })}
    >
      <Play aria-hidden />
      {started ? <ResumeProgress state={state} left={left} /> : "Play"}
    </Link>
  );
}

/** A Resume pill's name, saying what's left when that's known. */
function resumeName(left: string | undefined) {
  return left === undefined ? "Resume" : `Resume, ${left} left`;
}
