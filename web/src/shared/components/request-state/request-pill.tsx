import { type Request, RequestState } from "@/gen/nama/v1/request_pb";
import { DownloadingPill } from "@/shared/components/request-state/downloading-pill";
import { FailedPill } from "@/shared/components/request-state/failed-pill";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { StateLabel } from "@/shared/components/state-label";

/**
 * Where a Request stands, as a title shows it: filled to how far its downloads have got while
 * Processing with any, and red once Declined, or Failed, saying why. `large` beside a title's Play,
 * `onPopover` on a Popover such as the command menu.
 */
export function RequestPill({
  request: { state, downloads, failureReason },
  large = false,
  onPopover = false,
}: {
  request: Request;
  large?: boolean;
  onPopover?: boolean;
}) {
  if (state === RequestState.PROCESSING && downloads.length > 0) {
    return <DownloadingPill downloads={downloads} large={large} />;
  }
  if (state === RequestState.FAILED) {
    return <FailedPill reason={failureReason} large={large} onPopover={onPopover} />;
  }
  return <StateLabel {...shownOnTitle(state, onPopover)} large={large} />;
}
