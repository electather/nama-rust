import { EpisodeMoreLoader } from "@/features/title/components/episode-more-loader";
import type { ShowInLibrary } from "@/features/title/lib/show-params";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * An Episode card's "…", to mark it watched or delete it, only for one in the page's Show,
 * `show`.
 */
export function EpisodeCardMenu({
  show,
  item,
}: {
  show: ShowInLibrary | undefined;
  item: RowEpisode;
}) {
  // Without its own ID there's nothing to mark or delete.
  if (item.kind !== "here" || show === undefined || item.episode.id === undefined) {
    return null;
  }
  return <EpisodeMoreLoader show={show} episodeId={item.episode.id} episode={item.episode} />;
}
