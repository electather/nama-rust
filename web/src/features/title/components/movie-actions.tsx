import { useNavigate, useRouteContext } from "@tanstack/react-router";

import { CantDeleteMovie } from "@/features/title/components/cant-delete-movie";
import { EditImages } from "@/features/title/components/edit-images";
import { useSetPlayed } from "@/features/title/data/use-set-played";
import { movieImages } from "@/features/title/lib/title-images";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { PlayedButton } from "@/shared/components/watch-state/played-button";
import { deletedSearch } from "@/shared/lib/deleted-search";
import { may } from "@/shared/lib/permissions";

/**
 * What the signed-in User may do to `movie` of the Library `libraryId` besides playing it: mark it
 * played or unplayed, give it Custom Images with Manage Libraries, and delete it with Delete media
 * unless `readOnlyFolder` holds it.
 */
export function MovieActions({
  libraryId,
  movieId,
  movie,
  readOnlyFolder,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
  /** The folder holding the Movie that the Server couldn't write to when it last scanned. */
  readOnlyFolder: string | undefined;
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
      <EditImages title={movie.title} {...movieImages(movieId, movie)} />
      {may(signedIn.user, Permission.DELETE_MEDIA) &&
        (readOnlyFolder === undefined ? (
          <DeleteMedia
            name={`Delete ${movie.title}`}
            title={movie.title}
            versions={movie.versions}
            target={(versionId) =>
              versionId === undefined
                ? { target: { case: "movie", value: { libraryId, movieId } } }
                : { target: { case: "versionId", value: versionId } }
            }
            onDeletedEveryVersion={async (notTold) => {
              await navigate({
                to: "/libraries/$libraryId",
                params: { libraryId: String(libraryId) },
                search: deletedSearch(movie.title, notTold),
              });
            }}
          />
        ) : (
          <CantDeleteMovie title={movie.title} folder={readOnlyFolder} />
        ))}
    </>
  );
}
