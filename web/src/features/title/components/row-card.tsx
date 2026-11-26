import { EpisodeCard } from "@/features/title/components/episode-card";
import { FoundEpisodeCard } from "@/features/title/components/found-episode-card";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * One card of a Show's row, at `place`: an Episode of the page's Library, `libraryId`, which plays
 * it, or one it lacks.
 */
export function RowCard({
  libraryId,
  item,
  place,
}: {
  libraryId: bigint | undefined;
  item: RowEpisode;
  place: { index: number; of: number };
}) {
  if (item.kind !== "here") {
    return <FoundEpisodeCard item={item} place={place} />;
  }
  return (
    libraryId !== undefined && <EpisodeCard {...{ libraryId, place }} episode={item.episode} />
  );
}
