import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestStatus } from "@/shared/components/request-state/request-status";

/** `request`'s status, and the Quality it's downloaded at, if it has one. `what` it asks for. */
export function RequestStateAndQuality({ request, what }: { request: Request; what: string }) {
  return (
    <>
      <RequestStatus request={request} what={what} />
      {request.quality && (
        <span className="text-sm text-muted-foreground">{request.quality.name}</span>
      )}
    </>
  );
}
