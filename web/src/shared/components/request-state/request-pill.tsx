import { DownloadingPill } from "@/shared/components/request-state/downloading-pill";
import { FailedPill } from "@/shared/components/request-state/failed-pill";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { StateLabel } from "@/shared/components/state-label";
import type { RequestStanding } from "@/shared/lib/request-standing";

/**
 * Where a Request stands, by its `standing`, as a title shows it: filled to how far its downloads
 * have got while downloading, and red once stalled, saying why. `large` beside a title's Play,
 * `onPopover` on a Popover such as the command menu.
 */
export function RequestPill({
  standing,
  large = false,
  onPopover = false,
}: {
  standing: RequestStanding;
  large?: boolean;
  onPopover?: boolean;
}) {
  const { state, downloads, failureReason } = standing.request;
  if (standing.kind === "downloading") {
    return <DownloadingPill downloads={downloads} large={large} />;
  }
  if (standing.kind === "stalled") {
    return <FailedPill reason={failureReason} large={large} onPopover={onPopover} />;
  }
  return <StateLabel {...shownOnTitle(state, onPopover)} large={large} />;
}
