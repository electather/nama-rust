import { Link, type LinkOptions } from "@tanstack/react-router";
import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { ResumeProgress } from "@/shared/components/watch-state/resume-progress";
import { remainingOf } from "@/shared/lib/title-facts";
import { isStarted } from "@/shared/lib/watch-progress";
import { buttonVariants } from "@/shared/ui/button";

/**
 * The pill that plays `link`: "Play", or once started, how far in you are and what's left, such
 * as ▶ ━━── 33m, which it's named for, "Resume, 33m left". Named for `what` it plays too, such as
 * "Resume Season 2, Episode 3, 31m left", where the page doesn't say.
 */
export function PlayButton({
  link,
  state,
  what,
}: {
  link: LinkOptions;
  state: WatchState | undefined;
  what?: string;
}) {
  const started = isStarted(state);
  const left = remainingOf(state);
  return (
    <Link
      {...link}
      aria-label={started ? resumeName(left, what) : playName(what)}
      className={buttonVariants({ size: "pill" })}
    >
      <Play aria-hidden />
      {started ? <ResumeProgress state={state} left={left} /> : "Play"}
    </Link>
  );
}

/** A Resume pill's name, saying what it plays, and what's left when that's known. */
function resumeName(left: string | undefined, what: string | undefined) {
  const resume = what === undefined ? "Resume" : `Resume ${what}`;
  return left === undefined ? resume : `${resume}, ${left} left`;
}

/** A Play pill's name, when it says what it plays; else its text, "Play", names it. */
function playName(what: string | undefined) {
  return what === undefined ? undefined : `Play ${what}`;
}
