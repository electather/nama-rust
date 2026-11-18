import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { InYourLibrary } from "@/shared/components/found-movie-state/in-your-library";
import { RequestButton } from "@/shared/components/found-movie-state/request-button";
import { RequestStateAndQuality } from "@/shared/components/found-movie-state/request-state-and-quality";
import { RequestProgress } from "@/shared/components/request-progress/request-progress";
import { isRequester } from "@/shared/lib/is-requester";

/**
 * Where a Movie found in the title source is on its way to the Server: in a Library, linking
 * there, or its Request's state, and its downloads while Processing. Request, while the signed-in
 * User `you` isn't one of its Requesters, which `primary` makes the screen's primary action. The
 * caller lays them out.
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
      {request && <RequestStateAndQuality request={request} />}
      {!isRequester(request, you) && <RequestButton movie={movie} primary={primary} />}
      <RequestProgress request={request} />
    </>
  );
}
