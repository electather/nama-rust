import { useNavigate } from "@tanstack/react-router";
import type { Ref } from "react";

import { type ShowInLibrary, showParams } from "@/features/title/lib/show-params";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import type { Opener } from "@/shared/lib/opener";

/**
 * Deletes every Version of an Episode of `show`, or one of them, once `ref` opens it. Its card goes
 * with its last Version, so the URL of `show`'s page then says which Fulfillers may download it
 * again, keeping the rest of the page's search.
 */
export function DeleteEpisode({
  ref,
  show,
  episodeId,
  episode,
}: {
  ref: Ref<Opener>;
  show: ShowInLibrary;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const navigate = useNavigate();
  return (
    <DeleteMedia
      ref={ref}
      title={episode.title}
      versions={episode.versions}
      target={(versionId) => ({
        target: { case: "episode", value: { libraryId: show.libraryId, episodeId, versionId } },
      })}
      onDeletedEveryVersion={async (notTold) => {
        await navigate({
          to: "/libraries/$libraryId/shows/$showId",
          params: showParams(show),
          search: (shown) => ({ ...shown, ...deletedSearch(episode.title, notTold) }),
        });
      }}
    />
  );
}
