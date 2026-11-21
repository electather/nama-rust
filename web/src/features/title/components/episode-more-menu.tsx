import { useRef } from "react";

import { CardMoreActions } from "@/features/title/components/card-more-actions";
import { DeleteEpisode } from "@/features/title/components/delete-episode";
import { MarkPlayedItem } from "@/features/title/components/mark-played-item";
import { OpenerItem } from "@/features/title/components/opener-item";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/** An Episode card's "…": mark it watched or unwatched, and delete it with Delete media. */
export function EpisodeMoreMenu({
  libraryId,
  episodeId,
  episode,
}: {
  libraryId: bigint;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const remove = useRef<Opener>(null);
  const mayDelete = useMayDelete();
  return (
    <>
      <CardMoreActions label={`More for ${episode.title}`}>
        <MarkPlayedItem
          libraryId={libraryId}
          target={{ case: "episodeId", value: episodeId }}
          watchState={episode.watchState}
        />
        {mayDelete && <OpenerItem opener={remove}>Delete</OpenerItem>}
      </CardMoreActions>
      {/* Only with Delete media, so a page of Episodes doesn't hold a dialog for each in vain. */}
      {mayDelete && (
        <DeleteEpisode ref={remove} libraryId={libraryId} episodeId={episodeId} episode={episode} />
      )}
    </>
  );
}
