import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { InYourLibrary } from "@/shared/components/found-movie-state/in-your-library";
import { RequestButton } from "@/shared/components/found-movie-state/request-button";
import { RequestStateAndQuality } from "@/shared/components/found-movie-state/request-state-and-quality";
import { titleStanding } from "@/shared/lib/request-standing";

/**
 * Where a Movie found in the title source is on its way to the Server: in a Library, linking
 * there, or its Request's status. Request, which `primary` makes the screen's primary action, its
 * status as large, while the signed-in User `you` can ask for it or join. The caller lays them out.
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
  const { libraryId, request } = movie;
  if (libraryId !== undefined) {
    return <InYourLibrary movie={movie} libraryId={libraryId} />;
  }
  const standing = titleStanding({ request, inLibrary: false, you });
  return (
    <>
      {"request" in standing && (
        <RequestStateAndQuality standing={standing} what={movie.title} large={primary} />
      )}
      {standing.can !== "nothing" && (
        <RequestButton movie={movie} opening={standing.can === "open"} primary={primary} />
      )}
    </>
  );
}
