import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { RequestSplitButton } from "@/shared/components/request-button/request-split-button";
import { useAskForMovie } from "@/shared/hooks/use-ask-for-movie";
import { askRefusal } from "@/shared/lib/forms";

/**
 * Asks for `movie`, at the User's Quality, or one they pick while it's `opening` a Request rather
 * than joining one, then shows its Request once the Server has it.
 */
export function RequestButton({
  movie,
  opening,
  primary,
}: {
  movie: MovieHit | FoundMovie;
  opening: boolean;
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
        opening={opening}
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
