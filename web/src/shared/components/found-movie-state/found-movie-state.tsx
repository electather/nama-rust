import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { InYourLibrary } from "@/shared/components/found-movie-state/in-your-library";
import { RequestButton } from "@/shared/components/found-movie-state/request-button";
import { RequestStateAndQuality } from "@/shared/components/found-movie-state/request-state-and-quality";
import { canAsk } from "@/shared/lib/can-ask";

/**
 * Where a Movie found in the title source is on its way to the Server: in a Library, linking
 * there, or its Request's status. Request, which `primary` makes the screen's primary action,
 * while the signed-in User `you` can ask for it. The caller lays them out.
 */
export function FoundMovieState({
  movie,
  you,
  primary = false,
}: {
  movie: MovieHit | FoundMovie;
  you: bigint | undefined;
  primary?: boolean;
}) {
  if (movie.libraryId !== undefined) {
    return <InYourLibrary movie={movie} libraryId={movie.libraryId} />;
  }
  const { request } = movie;
  return (
    <>
      {request && <RequestStateAndQuality request={request} what={movie.title} />}
      {canAsk(request, you) && <RequestButton movie={movie} primary={primary} />}
    </>
  );
}
