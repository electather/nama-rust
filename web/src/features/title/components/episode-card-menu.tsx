import { EpisodeMoreLoader } from "@/features/title/components/episode-more-loader";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * An Episode card's "…", to mark it watched or delete it, only for one in the page's Library,
 * `libraryId`.
 */
export function EpisodeCardMenu({
  libraryId,
  item,
}: {
  libraryId: bigint | undefined;
  item: RowEpisode;
}) {
  // Without its own ID there's nothing to mark or delete.
  if (item.kind !== "here" || libraryId === undefined || item.episode.id === undefined) {
    return null;
  }
  return (
    <EpisodeMoreLoader libraryId={libraryId} episodeId={item.episode.id} episode={item.episode} />
  );
}
