import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";
import { useId } from "react";

import { RequestState } from "@/gen/nama/v1/request_pb";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { StateLabel } from "@/shared/components/state-label";

/**
 * "Failed" in red, with its Fulfiller's `reason`, if it gave one, on hover and on focus; `large`
 * beside a title's Play.
 */
export function FailedPill({ reason, large }: { reason: string | undefined; large: boolean }) {
  const reasonId = useId();
  const failed = <StateLabel {...shownOnTitle(RequestState.FAILED)} large={large} />;
  if (reason === undefined) {
    return failed;
  }
  return (
    <Tooltip>
      {/* A screen reader hears the reason as the pill's description, open or not. */}
      <TooltipTrigger aria-describedby={reasonId}>{failed}</TooltipTrigger>
      <span id={reasonId} hidden>
        {reason}
      </span>
      <TooltipContent>{reason}</TooltipContent>
    </Tooltip>
  );
}
