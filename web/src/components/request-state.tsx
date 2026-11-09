import { useQueryClient } from "@tanstack/react-query";
import {
  Check,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  Clock,
  Download,
  TriangleAlert,
  X,
} from "lucide-react";

import { RequestProgress } from "@/components/request-progress";
// Where a Request is on its way, so a screen showing its state shows its downloads from here too.
export { RequestProgress };
import { type Shown, StateLabel } from "@/components/state-label";
import { type Request, RequestState } from "@/gen/nama/v1/request_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { requestsKey } from "@/lib/events";
import { refusalOf } from "@/lib/forms";

/** Each state in the Glossary's words, with its icon, so it never rests on colour alone. */
const states: Partial<Record<RequestState, Shown>> = {
  [RequestState.PENDING]: { label: "Pending", Icon: Clock },
  [RequestState.APPROVED]: { label: "Approved", Icon: Check },
  [RequestState.DECLINED]: { label: "Declined", Icon: X },
  [RequestState.PROCESSING]: { label: "Processing", Icon: Download },
  [RequestState.PARTIALLY_AVAILABLE]: { label: "Partially Available", Icon: CircleDashed },
  [RequestState.AVAILABLE]: { label: "Available", Icon: CircleCheck },
  [RequestState.FAILED]: { label: "Failed", Icon: TriangleAlert },
};

/** A state from a newer Server than this app knows. */
const unknown: Shown = { label: "Unknown", Icon: CircleHelp };

/** A Request's state in the Glossary's words. */
export function requestStateName(state: RequestState) {
  return (states[state] ?? unknown).label;
}

/** A Request's state as a labelled pill. */
export function RequestStateLabel({ state }: { state: RequestState }) {
  return <StateLabel {...(states[state] ?? unknown)} />;
}

/** Whether the signed-in User `you` is one of the Requesters of `request`, or of a Followed Show. */
export function isRequester(
  request: Pick<Request, "requesters"> | undefined,
  you: bigint | undefined,
) {
  return request?.requesters.some(({ userId }) => userId === you) ?? false;
}

/**
 * Mutation options for asking for a title, which read the Requests back once it's asked for, or
 * once it's refused because a scan Matched it since it was found, to show it in its Library.
 */
export function useReadBackAsked() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  const readBack = () => queryClient.invalidateQueries({ queryKey: requestsKey });
  return {
    onSuccess: readBack,
    onError: (error: unknown) =>
      refusalOf(error).reason === RefusalReason.ALREADY_IN_LIBRARY ? readBack() : undefined,
  };
}

/** Mutation options that refetch the Requests once a change to one is done or refused. */
export function useRefetchRequests() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  return { onSettled: () => queryClient.invalidateQueries({ queryKey: requestsKey }) };
}
