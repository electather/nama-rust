import { Button } from "nama-ui/components/button";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import { useRetryRequest } from "@/shared/hooks/use-retry-request";
import { refusal } from "@/shared/lib/forms";
import type { RequestStanding } from "@/shared/lib/request-standing";

/**
 * Retry, which hands the Request for `what` to its Fulfiller again while its `standing` is stalled,
 * for a Request manager. Nothing otherwise.
 */
export function RetryRequest({ standing, what }: { standing: RequestStanding; what: string }) {
  const mayRetry = useMay(Permission.MANAGE_REQUESTS);
  const retry = useRetryRequest();
  if (!mayRetry || standing.kind !== "stalled") {
    return null;
  }
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`Retry ${what}`}
        disabled={retry.isPending}
        onClick={() => {
          retry.mutate({ id: standing.request.id });
        }}
      >
        Retry
      </Button>
      {retry.isError && (
        <p role="alert" className="w-full text-sm text-destructive">
          {
            refusal(
              retry.error,
              "Retry this Request",
              {},
              {
                [RefusalReason.REQUEST_NOT_FAILED]: { form: `${what} isn’t Failed any more.` },
              },
            ).form
          }
        </p>
      )}
    </>
  );
}
