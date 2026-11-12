import { useNavigate, useRouteContext } from "@tanstack/react-router";

import { DeleteMedia } from "@/components/delete-media";
import { PlayedButton } from "@/components/watch-state";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { deletedSearch } from "@/lib/browse";
import { may } from "@/lib/permissions";
import { useSetPlayed } from "@/lib/watch-state";

/**
 * What the signed-in User may do to `movie` of the Library `libraryId` besides playing it: mark it
 * played or unplayed, and delete it with Delete media.
 */
export function MovieActions({
  libraryId,
  movieId,
  movie,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
}) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const navigate = useNavigate();
  const setPlayed = useSetPlayed();
  return (
    <>
      <PlayedButton
        played={movie.watchState?.played === true}
        onToggle={(played) => {
          setPlayed({ libraryId, target: { case: "movieId", value: movieId }, played });
        }}
      />
      {may(signedIn.user, Permission.DELETE_MEDIA) && (
        <DeleteMedia
          name={`Delete ${movie.title}`}
          title={movie.title}
          versions={movie.versions}
          target={(versionId) =>
            versionId === undefined
              ? { target: { case: "movie", value: { libraryId, movieId } } }
              : { target: { case: "versionId", value: versionId } }
          }
          onDeletedEveryVersion={(notTold) =>
            navigate({
              to: "/libraries/$libraryId",
              params: { libraryId: String(libraryId) },
              search: deletedSearch(movie.title, notTold),
            })
          }
        />
      )}
    </>
  );
}
