import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestStateLabel } from "@/shared/components/request-state/request-state-label";

/** `request`'s state, and the Quality it's downloaded at, if it has one. */
export function RequestStateAndQuality({ request }: { request: Request }) {
  return (
    <>
      <RequestStateLabel state={request.state} />
      {request.quality && (
        <span className="text-sm text-muted-foreground">{request.quality.name}</span>
      )}
    </>
  );
}
