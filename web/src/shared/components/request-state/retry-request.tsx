import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { useRetryRequest } from "@/shared/hooks/use-retry-request";
import { refusal } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/** Hands the Failed Request `id` for `what` to its Fulfiller again, for a Request manager. */
export function RetryRequest({ id, what }: { id: bigint; what: string }) {
  const retry = useRetryRequest();
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`Retry ${what}`}
        disabled={retry.isPending}
        onClick={() => {
          retry.mutate({ id });
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
