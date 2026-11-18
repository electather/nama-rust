import { EpisodeCard } from "@/features/title/components/episode-card";
import { useEpisodes } from "@/features/title/data/use-episodes";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { BackdropGrid } from "@/shared/components/backdrop-grid";

/** The Episodes of the Season `seasonNumber`, each a card. */
export function Episodes({
  libraryId,
  showId,
  seasonNumber,
}: {
  libraryId: bigint;
  showId: ExternalId;
  seasonNumber: number;
}) {
  const data = useEpisodes({ libraryId, showId, seasonNumber });
  return (
    <BackdropGrid>
      {data.episodes.map((episode) => (
        <EpisodeCard key={episode.episodeNumber} libraryId={libraryId} episode={episode} />
      ))}
    </BackdropGrid>
  );
}
