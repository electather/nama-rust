import { useParams } from "@tanstack/react-router";

import { EpisodeCaption } from "@/features/title/components/episode-caption";
import { EpisodeMoreLoader } from "@/features/title/components/episode-more-loader";
import { episodeCardLabel } from "@/features/title/lib/episode-card-label";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { BackdropCard } from "@/shared/components/backdrop-card/backdrop-card";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";

/**
 * An Episode by its Still, which plays its largest Version, with its caption over
 * the Still and a "…" to mark it watched or delete it, as Continue Watching's cards are drawn.
 */
export function EpisodeCard({ libraryId, episode }: { libraryId: bigint; episode: Episode }) {
  // Without its own ID there's nothing to mark or delete.
  const episodeId = episode.id;
  const show = useParams({ from: "/_member/libraries/$libraryId/shows/$showId/" });
  return (
    <li data-season={episode.seasonNumber} className="w-tile-backdrop shrink-0 snap-start">
      <BackdropCard
        art={{ url: episode.stillUrl, thumbhash: episode.stillThumbhash, sizes: backdropTileSizes }}
        link={{
          to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
          params: {
            ...show,
            season: String(episode.seasonNumber),
            episode: String(episode.episodeNumber),
          },
        }}
        label={episodeCardLabel(episode)}
        tall={episode.overview !== undefined}
        caption={<EpisodeCaption episode={episode} />}
        menu={
          episodeId && (
            <EpisodeMoreLoader libraryId={libraryId} episodeId={episodeId} episode={episode} />
          )
        }
      />
    </li>
  );
}
