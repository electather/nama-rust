import { Approve } from "@/features/requests/components/approve";
import { Cancel } from "@/features/requests/components/cancel";
import { Decline } from "@/features/requests/components/decline";
import { Retry } from "@/features/requests/components/retry";
import type { Target } from "@/features/requests/lib/request-target";
import { RequestState } from "@/gen/nama/v1/request_pb";

/** Approve and Decline for a Pending Request, Retry for a Failed one, and Cancel for any. */
export function Manage({ state, ...target }: Target & { state: RequestState }) {
  return (
    <>
      {state === RequestState.PENDING && (
        <>
          <Approve {...target} />
          <Decline {...target} />
        </>
      )}
      {state === RequestState.FAILED && <Retry {...target} />}
      <Cancel {...target} />
    </>
  );
}
