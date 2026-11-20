import { useNavigate } from "@tanstack/react-router";
import type { Ref } from "react";

import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import type { Opener } from "@/shared/lib/opener";

/**
 * Deletes an Episode's every Version, or one of them, once `ref` opens it. Its
 * card goes with its last Version, so the page's URL then says which Fulfillers may download it
 * again.
 */
export function DeleteEpisode({
  ref,
  libraryId,
  episodeId,
  episode,
}: {
  ref: Ref<Opener>;
  libraryId: bigint;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const navigate = useNavigate({ from: "/libraries/$libraryId/shows/$showId/" });
  return (
    <DeleteMedia
      ref={ref}
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
