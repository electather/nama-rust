import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestPill } from "@/shared/components/request-state/request-pill";
import { RetryRequest } from "@/shared/components/request-state/retry-request";

/** `request`'s pill, Retry for it, and the Quality it's downloaded at, if it has one. `what` it asks for. */
export function RequestStateAndQuality({ request, what }: { request: Request; what: string }) {
  return (
    <>
      <RequestPill request={request} />
      <RetryRequest request={request} what={what} />
      {request.quality && (
        <span className="text-sm text-muted-foreground">{request.quality.name}</span>
      )}
    </>
  );
}
