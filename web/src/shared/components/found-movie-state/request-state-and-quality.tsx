import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestPill } from "@/shared/components/request-state/request-pill";
import { RetryRequest } from "@/shared/components/request-state/retry-request";

/**
 * `request`'s pill, `large` beside a title's Play, Retry for it, and the Quality it's downloaded
 * at, if it has one. `what` it asks for.
 */
export function RequestStateAndQuality({
  request,
  what,
  large = false,
}: {
  request: Request;
  what: string;
  large?: boolean;
}) {
  return (
    <>
      <RequestPill request={request} large={large} />
      <RetryRequest request={request} what={what} />
      {request.quality && (
        <span className="text-sm text-muted-foreground">{request.quality.name}</span>
      )}
    </>
  );
}
