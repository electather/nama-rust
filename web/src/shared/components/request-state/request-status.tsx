import { type Request, RequestState } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { RequestPill } from "@/shared/components/request-state/request-pill";
import { RetryRequest } from "@/shared/components/request-state/retry-request";
import { useMay } from "@/shared/hooks/use-may";

/** `request`'s pill, and Retry beside it once Failed, for a Request manager. `what` it asks for. */
export function RequestStatus({ request, what }: { request: Request; what: string }) {
  const mayRetry = useMay(Permission.MANAGE_REQUESTS);
  return (
    <>
      <RequestPill request={request} />
      {mayRetry && request.state === RequestState.FAILED && (
        <RetryRequest id={request.id} what={what} />
      )}
    </>
  );
}
