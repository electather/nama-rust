import { EpisodeCard } from "@/features/title/components/episode-card";
import { FoundEpisodeCard } from "@/features/title/components/found-episode-card";
import type { RowLook } from "@/features/title/lib/row-look";
import type { ShowInLibrary } from "@/features/title/lib/show-params";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * One card of a Show's row, at `place`: an Episode of the page's Show `show`, which
 * plays it, or one it lacks, drawn as the row's `look` says.
 */
export function RowCard({
  show,
  item,
  place,
  look,
}: {
  show: ShowInLibrary | undefined;
  item: RowEpisode;
  place: { index: number; of: number };
  look: RowLook;
}) {
  if (item.kind !== "here") {
    return <FoundEpisodeCard {...{ item, place, look }} />;
  }
  return show !== undefined && <EpisodeCard {...{ show, place, look }} episode={item.episode} />;
}
