import { useNavigate } from "@tanstack/react-router";
import type { Ref } from "react";

import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import type { Opener } from "@/shared/lib/opener";

/**
 * Deletes `movie`, or one of its Versions, once `ref` opens it and it's confirmed, then leaves for
 * its Library once every Version went, saying which Fulfillers may download it again.
 */
export function DeleteMovie({
  ref,
  libraryId,
  movieId,
  movie,
}: {
  ref: Ref<Opener>;
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
}) {
  const navigate = useNavigate();
  // Without Delete media, the page holds no dialog it can't open.
  if (!useMayDelete()) {
    return null;
  }
  return (
    <DeleteMedia
      ref={ref}
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
  );
}
