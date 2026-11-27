import { EpisodeCard } from "@/features/title/components/episode-card";
import { FoundEpisodeCard } from "@/features/title/components/found-episode-card";
import type { RowLook } from "@/features/title/lib/row-look";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * One card of a Show's row, at `place`: an Episode of the page's Library, `libraryId`, which plays
 * it, or one it lacks, drawn as the row's `look` says.
 */
export function RowCard({
  libraryId,
  item,
  place,
  look,
}: {
  libraryId: bigint | undefined;
  item: RowEpisode;
  place: { index: number; of: number };
  look: RowLook;
}) {
  if (item.kind !== "here") {
    return <FoundEpisodeCard {...{ item, place, look }} />;
  }
  return (
    libraryId !== undefined && (
      <EpisodeCard {...{ libraryId, place, look }} episode={item.episode} />
    )
  );
}
