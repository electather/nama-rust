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
  type LucideIcon,
  Plus,
  TriangleAlert,
  X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  type FoundMovie,
  type Request,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { requestsKey } from "@/lib/events";
import { refusal } from "@/lib/forms";

interface Shown {
  label: string;
  Icon: LucideIcon;
}

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
 * Request's state. Request, while the signed-in User `you` isn't one of its Requesters, which
 * `primary` makes the screen's primary action. The caller lays them out.
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
    </>
  );
}

function isRequester(request: Request | undefined, you: bigint | undefined) {
  return request?.requesters.some(({ userId }) => userId === you) ?? false;
}

/** Asks for `movie`, then shows its Request once the Server has it. */
function RequestButton({ movie, primary }: { movie: FoundMovie; primary: boolean }) {
  const queryClient = useQueryClient();
  const ask = useMutation(RequestService.method.createRequest, {
    // Waits for the Request to be read back, so the button stays busy until its state shows.
    onSuccess: () => queryClient.invalidateQueries({ queryKey: requestsKey }),
  });
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
          {refusal(ask.error, `Request ${movie.title}`, {}).form}
        </p>
      )}
    </>
  );
}

function StateLabel({ label, Icon }: Shown) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-sm text-muted-foreground">
      <Icon aria-hidden className="size-3.5" />
      {label}
    </span>
  );
}
