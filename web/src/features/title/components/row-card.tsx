import { EpisodeCard } from "@/features/title/components/episode-card";
import { FoundEpisodeCard } from "@/features/title/components/found-episode-card";
import type { RowLook } from "@/features/title/lib/row-look";
import type { RowEpisode } from "@/features/title/lib/show-row";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * One card of a Show's row, at `place`: an Episode of the page's Library Show, `show`, which
 * plays it, or one it lacks, drawn as the row's `look` says.
 */
export function RowCard({
  show,
  item,
  place,
  look,
}: {
  show: { libraryId: bigint; showId: ExternalId } | undefined;
  item: RowEpisode;
  place: { index: number; of: number };
  look: RowLook;
}) {
  if (item.kind !== "here") {
    return <FoundEpisodeCard {...{ item, place, look }} />;
  }
  return show !== undefined && <EpisodeCard {...{ show, place, look }} episode={item.episode} />;
}
