import { useNavigate } from "@tanstack/react-router";

import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";

/**
 * With Delete media, deletes an Episode's every Version, or one of them. Its row goes with its
 * last Version, so the page's URL then says which Fulfillers may download it again.
 */
export function DeleteEpisode({
  libraryId,
  episodeId,
  episode,
}: {
  libraryId: bigint;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const navigate = useNavigate({ from: "/libraries/$libraryId/shows/$showId/" });
  if (!useMayDelete()) {
    return null;
  }
  return (
    <DeleteMedia
      name={`Delete ${episode.title}`}
      title={episode.title}
      versions={episode.versions}
      target={(versionId) => ({
        target: { case: "episode", value: { libraryId, episodeId, versionId } },
      })}
      onDeletedEveryVersion={async (notTold) => {
        await navigate({
          search: (shown) => ({ ...shown, ...deletedSearch(episode.title, notTold) }),
        });
      }}
    />
  );
}
