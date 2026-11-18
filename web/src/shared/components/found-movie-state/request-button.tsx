import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { RequestMovieButton } from "@/shared/components/found-movie-state/request-movie-button";
import { QualityPicker } from "@/shared/components/quality-picker/quality-picker";
import { useAskForMovie } from "@/shared/hooks/use-ask-for-movie";
import { useQualityPicker } from "@/shared/hooks/use-quality-picker";
import { askRefusal } from "@/shared/lib/forms";

/**
 * Asks for `movie`, then shows its Request once the Server has it. Opening a Request asks in a
 * dialog which Quality to download it at, when the signed-in User may choose and there's a choice.
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
  const picker = useQualityPicker(movie.request === undefined, "movie");
  const whyRefused = (error: unknown) =>
    askRefusal(error, `Request ${movie.title}`, {
      [RefusalReason.ALREADY_IN_LIBRARY]: { form: `${movie.title} is already in a Library.` },
      [RefusalReason.QUALITY_NOT_OFFERED]: {
        form: "That Quality isn’t offered any more. Reload the page and pick another.",
      },
    });
  return (
    <>
      <RequestMovieButton
        title={movie.title}
        primary={primary}
        hasPopup={picker.hasPopup}
        disabled={ask.isPending || picker.loading}
        onRequest={() => {
          if (!picker.open()) {
            ask.mutate({ movieId });
          }
        }}
      />
      <QualityPicker
        {...picker.dialog}
        title={movie.title}
        onPick={async (quality) => {
          await ask.mutateAsync({ movieId, quality });
        }}
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
