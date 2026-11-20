import type { RequestState } from "@/gen/nama/v1/request_pb";
import { shownRequestState } from "@/shared/components/request-state/request-states";
import { StateLabel } from "@/shared/components/state-label";

/** A Request's state as a labelled pill. */
export function RequestStateLabel({ state }: { state: RequestState }) {
  return <StateLabel {...shownRequestState(state)} />;
}
