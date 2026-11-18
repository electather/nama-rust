import { Link, type LinkOptions } from "@tanstack/react-router";
import { Play } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { ResumeDetail } from "@/shared/components/watch-state/resume-detail";
import { isStarted } from "@/shared/lib/watch-progress";
import { buttonVariants } from "@/shared/ui/button";

/**
 * The pill at the top of a title's page that plays `link`: Play, or once started, Resume with a
 * bar showing how far in you are and `detail`, such as how much is left.
 */
export function PlayOrResume({
  link,
  state,
  detail,
}: {
  link: LinkOptions;
  state: WatchState | undefined;
  detail: string | undefined;
}) {
  const started = isStarted(state);
  return (
    <>
      <Link {...link} className={buttonVariants({ size: "pill" })}>
        <Play aria-hidden />
        {started ? "Resume" : "Play"}
      </Link>
      {started && <ResumeDetail state={state} detail={detail} />}
    </>
  );
}
