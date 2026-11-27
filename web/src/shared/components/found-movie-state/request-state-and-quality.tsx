import { RequestPill } from "@/shared/components/request-state/request-pill";
import { RetryRequest } from "@/shared/components/request-state/retry-request";
import type { RequestStanding } from "@/shared/lib/request-standing";

/**
 * The pill of a Request by its `standing`, `large` beside a title's Play, Retry for it, and the
 * Quality it's downloaded at, if it has one. `what` it asks for.
 */
export function RequestStateAndQuality({
  standing,
  what,
  large = false,
}: {
  standing: RequestStanding;
  what: string;
  large?: boolean;
}) {
  return (
    <>
      <RequestPill standing={standing} large={large} />
      <RetryRequest standing={standing} what={what} />
      {standing.request.quality && (
        <span className="text-sm text-muted-foreground">{standing.request.quality.name}</span>
      )}
    </>
  );
}
