import { DeleteEpisode } from "@/features/title/components/delete-episode";
import { EpisodeLink } from "@/features/title/components/episode-link";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { PlayedIconButton } from "@/shared/components/watch-state/played-icon-button";
import { useSetPlayed } from "@/shared/hooks/use-set-played";

/**
 * An Episode by its Still, which plays its largest Version, with that Version's picture and
 * sound, your Watch State and a toggle for it.
 */
export function EpisodeCard({ libraryId, episode }: { libraryId: bigint; episode: Episode }) {
  const setPlayed = useSetPlayed();
  // Without its own ID there's nothing to mark or delete.
  const episodeId = episode.id;
  return (
    <li className="flex flex-col">
      <EpisodeLink episode={episode} />
      {episodeId && (
        // Pulled out by the buttons' own padding, so the first icon lines up with the text.
        <div className="-ms-2 mt-1 flex">
          <PlayedIconButton
            played={episode.watchState?.played === true}
            label={`Watched: ${episode.title}`}
            onToggle={(played) => {
              setPlayed({ libraryId, target: { case: "episodeId", value: episodeId }, played });
            }}
          />
          <DeleteEpisode libraryId={libraryId} episodeId={episodeId} episode={episode} />
        </div>
      )}
    </li>
  );
}
