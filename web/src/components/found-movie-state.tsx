import { useMutation } from "@connectrpc/connect-query";
import { Library, Plus } from "lucide-react";

import { QualityPicker, useQualityPicker } from "@/components/quality-picker";
import {
  RequestProgress,
  RequestStateLabel,
  isRequester,
  useReadBackAsked,
} from "@/components/request-state";
import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FoundMovie, type Request, RequestService } from "@/gen/nama/v1/request_pb";
import { askRefusal } from "@/lib/forms";

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
      {request && <RequestStateAndQuality request={request} />}
      {!isRequester(request, you) && <RequestButton movie={movie} primary={primary} />}
      <RequestProgress request={request} />
    </>
  );
}

/** `request`'s state, and the Quality it's downloaded at, if it has one. */
function RequestStateAndQuality({ request }: { request: Request }) {
  return (
    <>
      <RequestStateLabel state={request.state} />
      {request.quality && (
        <span className="text-sm text-muted-foreground">{request.quality.name}</span>
      )}
    </>
  );
}

/**
 * Asks for `movie`, then shows its Request once the Server has it. Opening a Request asks in a
 * dialog which Quality to download it at, when the signed-in User may choose and there's a choice.
 */
function RequestButton({ movie, primary }: { movie: FoundMovie; primary: boolean }) {
  const ask = useMutation(RequestService.method.createRequest, useReadBackAsked());
  const picker = useQualityPicker(movie.request === undefined);
  const whyRefused = (error: unknown) =>
    askRefusal(error, `Request ${movie.title}`, {
      [RefusalReason.ALREADY_IN_LIBRARY]: { form: `${movie.title} is already in a Library.` },
      [RefusalReason.QUALITY_NOT_OFFERED]: {
        form: "That Quality isn’t offered any more. Reload the page and pick another.",
      },
    });
  // A ghost button's icon lines up with the caption above it, its padding hanging outside.
  const [variant, hanging] = primary ? (["default"] as const) : (["ghost", "-ms-4"] as const);
  return (
    <>
      <span className={hanging}>
        <Button
          variant={variant}
          aria-label={`Request ${movie.title}`}
          aria-haspopup={picker.hasPopup}
          disabled={ask.isPending || picker.loading}
          onClick={() => {
            if (!picker.open()) ask.mutate({ movieTmdbId: movie.tmdbId });
          }}
        >
          <Plus aria-hidden className="size-4" />
          Request
        </Button>
      </span>
      <QualityPicker
        {...picker.dialog}
        title={movie.title}
        onPick={(quality) => ask.mutateAsync({ movieTmdbId: movie.tmdbId, quality })}
        refusal={whyRefused}
      />
      {ask.isError && (
        <p role="alert" className="w-full text-sm text-destructive">
          {whyRefused(ask.error)}
        </p>
      )}
    </>
  );
}
