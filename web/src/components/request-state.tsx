import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import {
  Check,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  Clock,
  Download,
  Library,
  Plus,
  TriangleAlert,
  X,
} from "lucide-react";

import { RequestProgress } from "@/components/request-progress";
// Where a Request is on its way, so a screen showing its state shows its downloads from here too.
export { RequestProgress };
import { type Shown, StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import {
  type FoundMovie,
  type Request,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { requestsKey } from "@/lib/events";
import { askRefusal, refusalOf } from "@/lib/forms";

/** Each state in the Glossary's words, with its icon, so it never rests on colour alone. */
const states: Partial<Record<RequestState, Shown>> = {
  [RequestState.PENDING]: { label: "Pending", Icon: Clock },
  [RequestState.APPROVED]: { label: "Approved", Icon: Check },
  [RequestState.DECLINED]: { label: "Declined", Icon: X },
  [RequestState.PROCESSING]: { label: "Processing", Icon: Download },
  [RequestState.PARTIALLY_AVAILABLE]: { label: "Partially Available", Icon: CircleDashed },
  [RequestState.AVAILABLE]: { label: "Available", Icon: CircleCheck },
  [RequestState.FAILED]: { label: "Failed", Icon: TriangleAlert },
};

/** A state from a newer Server than this app knows. */
const unknown: Shown = { label: "Unknown", Icon: CircleHelp };

/** A Request's state as a labelled pill. */
export function RequestStateLabel({ state }: { state: RequestState }) {
  return <StateLabel {...(states[state] ?? unknown)} />;
}

/**
 * Where a Movie found in the metadata source is on its way to the Server: in a Library, or its
 * Request's state, and its downloads while Processing. Request, while the signed-in User `you`
 * isn't one of its Requesters, which `primary` makes the screen's primary action. The caller lays
 * them out.
 */
export function FoundMovieState({
  movie,
  you,
  primary = false,
}: {
  movie: FoundMovie;
  you: bigint | undefined;
  primary?: boolean;
}) {
  if (movie.libraryId !== undefined) return <StateLabel label="In a Library" Icon={Library} />;
  const { request } = movie;
  return (
    <>
      {request && <RequestStateLabel state={request.state} />}
      {!isRequester(request, you) && <RequestButton movie={movie} primary={primary} />}
      <RequestProgress request={request} />
    </>
  );
}

/** Whether the signed-in User `you` is one of the Requesters of `request`, or of a Followed Show. */
export function isRequester(
  request: Pick<Request, "requesters"> | undefined,
  you: bigint | undefined,
) {
  return request?.requesters.some(({ userId }) => userId === you) ?? false;
}

/**
 * Mutation options for asking for a title, which read the Requests back once it's asked for, or
 * once it's refused because a scan Matched it since it was found, to show it in its Library.
 */
export function useReadBackAsked() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  const readBack = () => queryClient.invalidateQueries({ queryKey: requestsKey });
  return {
    onSuccess: readBack,
    onError: (error: unknown) =>
      refusalOf(error).reason === RefusalReason.ALREADY_IN_LIBRARY ? readBack() : undefined,
  };
}

/** Mutation options that refetch the Requests once a change to one is done or refused. */
export function useRefetchRequests() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  return { onSettled: () => queryClient.invalidateQueries({ queryKey: requestsKey }) };
}

/** Asks for `movie`, then shows its Request once the Server has it. */
function RequestButton({ movie, primary }: { movie: FoundMovie; primary: boolean }) {
  const ask = useMutation(RequestService.method.createRequest, useReadBackAsked());
  return (
    <>
      {/* A ghost button's icon lines up with the caption above it, its padding hanging outside. */}
      <span className={primary ? undefined : "-ms-4"}>
        <Button
          variant={primary ? "default" : "ghost"}
          aria-label={`Request ${movie.title}`}
          disabled={ask.isPending}
          onClick={() => {
            ask.mutate({ movieTmdbId: movie.tmdbId });
          }}
        >
          <Plus aria-hidden className="size-4" />
          Request
        </Button>
      </span>
      {ask.isError && (
        <p role="alert" className="w-full text-sm text-destructive">
          {askRefusal(ask.error, `Request ${movie.title}`, {
            [RefusalReason.ALREADY_IN_LIBRARY]: { form: `${movie.title} is already in a Library.` },
          })}
        </p>
      )}
    </>
  );
}
