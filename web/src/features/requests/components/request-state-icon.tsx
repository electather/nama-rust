import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { RequestState } from "@/gen/nama/v1/request_pb";
import { shownOnTitle, shownRequestState } from "@/shared/components/request-state/request-states";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/** A Request's state as its icon alone, named on hover and on focus. */
export function RequestStateIcon({ state }: { state: RequestState }) {
  const { label, Icon } = shownRequestState(state);
  const [trigger, container] = useModalContainer();
  return (
    <Tooltip>
      {/* Not a button, as pressing it does nothing; focusable so a keyboard opens its tooltip. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- see above */}
      <TooltipTrigger render={<span ref={trigger} tabIndex={0} />}>
        <Icon aria-hidden strokeWidth={1.5} className={`block size-3.5 ${toneOf(state)}`} />
        <span className="sr-only">{label}</span>
      </TooltipTrigger>
      <TooltipContent container={container}>{label}</TooltipContent>
    </Tooltip>
  );
}

/** Red once turned down or stalled, lit once Available, quiet otherwise. */
function toneOf(state: RequestState) {
  if (shownOnTitle(state).tone === "destructive") {
    return "text-destructive";
  }
  return state === RequestState.AVAILABLE ? "text-primary" : "text-muted-foreground";
}
