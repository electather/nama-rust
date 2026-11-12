import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useId } from "react";

import { DeleteMedia } from "@/components/delete-media";
import { Button } from "@/components/ui/button";
import { PlayedButton } from "@/components/watch-state";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { deletedSearch } from "@/lib/browse";
import { may } from "@/lib/permissions";
// oxlint-disable-next-line import/max-dependencies -- the merged Movie actions need both the read-only notice and the delete report
import { useSetPlayed } from "@/lib/watch-state";

/**
 * What the signed-in User may do to `movie` of the Library `libraryId` besides playing it: mark it
 * played or unplayed, and delete it with Delete media unless `readOnlyFolder` holds it.
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
            onDeletedEveryVersion={(notTold) =>
              navigate({
                to: "/libraries/$libraryId",
                params: { libraryId: String(libraryId) },
                search: deletedSearch(movie.title, notTold),
              })
            }
          />
        ) : (
          <CantDeleteMovie title={movie.title} folder={readOnlyFolder} />
        ))}
    </>
  );
}

/** The Delete action of the Movie `title`, disabled, saying the Server can't write to `folder`. */
function CantDeleteMovie({ title, folder }: { title: string; folder: string }) {
  const reason = useId();
  return (
    <>
      <Button variant="ghost" disabled aria-label={`Delete ${title}`} aria-describedby={reason}>
        Delete
      </Button>
      <p id={reason} className="basis-full text-sm break-words text-muted-foreground">
        {title} can’t be deleted: the Server can’t write to {folder}. Make that folder writable,
        then scan the Library again.
      </p>
    </>
  );
}
