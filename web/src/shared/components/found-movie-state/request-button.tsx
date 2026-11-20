import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FoundMovie, type MovieHit, RequestState } from "@/gen/nama/v1/request_pb";
import { RequestSplitButton } from "@/shared/components/request-button/request-split-button";
import { useAskForMovie } from "@/shared/hooks/use-ask-for-movie";
import { askRefusal } from "@/shared/lib/forms";

/**
 * Asks for `movie`, at the User's Quality or one they pick, then shows its Request once the Server
 * has it. Asking for one Declined opens a new Request, so its Quality can be picked again.
 */
export function RequestButton({
  movie,
  primary,
}: {
  movie: MovieHit | FoundMovie;
  primary: boolean;
}) {
  const movieId = movie.id;
  const ask = useAskForMovie();
  const whyRefused = (error: unknown) =>
    askRefusal(error, `Request ${movie.title}`, {
      [RefusalReason.ALREADY_IN_LIBRARY]: { form: `${movie.title} is already in a Library.` },
      [RefusalReason.QUALITY_NOT_OFFERED]: {
        form: "That Quality isn’t offered any more. Reload the page and pick another.",
      },
    });
  return (
    <>
      <RequestSplitButton
        what={movie.title}
        kind="movie"
        opening={movie.request === undefined || movie.request.state === RequestState.DECLINED}
        primary={primary}
        busy={ask.isPending}
        onRequest={(quality) => {
          ask.mutate({ movieId, quality });
        }}
      />
      {ask.isError && (
        <p role="alert" className="w-full text-sm text-destructive">
          {whyRefused(ask.error)}
        </p>
      )}
    </>
  );
}
