import { useRef } from "react";

import { CardMoreActions } from "@/features/title/components/card-more-actions";
import { DeleteEpisode } from "@/features/title/components/delete-episode";
import { MarkPlayedItem } from "@/features/title/components/mark-played-item";
import { OpenerItem } from "@/features/title/components/opener-item";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { ShowInLibrary } from "@/features/title/lib/show-params";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/** The "…" of a card of an Episode of `show`: mark it watched or unwatched, and delete it with Delete media. */
export function EpisodeMoreMenu({
  show,
  episodeId,
  episode,
}: {
  show: ShowInLibrary;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const remove = useRef<Opener>(null);
  const mayDelete = useMayDelete();
  return (
    <>
      <CardMoreActions label={`More for ${episode.title}`}>
        <MarkPlayedItem
          libraryId={show.libraryId}
          target={{ case: "episodeId", value: episodeId }}
          watchState={episode.watchState}
        />
        {mayDelete && <OpenerItem opener={remove}>Delete</OpenerItem>}
      </CardMoreActions>
      {/* Only with Delete media, so a page of Episodes doesn't hold a dialog for each in vain. */}
      {mayDelete && (
        <DeleteEpisode ref={remove} show={show} episodeId={episodeId} episode={episode} />
      )}
    </>
  );
}
