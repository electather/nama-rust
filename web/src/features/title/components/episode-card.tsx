import { EpisodeCaption } from "@/features/title/components/episode-caption";
import { EpisodeMoreLoader } from "@/features/title/components/episode-more-loader";
import { episodeCardLabel } from "@/features/title/lib/episode-card-label";
import type { RowLook } from "@/features/title/lib/row-look";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { BackdropCard } from "@/shared/components/backdrop-card/backdrop-card";
import { stillOf } from "@/shared/lib/server-image";
import { backdropTile } from "@/shared/lib/tile-shapes";
import { titleParam } from "@/shared/lib/title-param";

/**
 * An Episode of the Library Show `show` by its Still, which plays its largest Version, with its caption over
 * the Still and a "…" to mark it watched or delete it, as Continue Watching's cards are drawn. It is
 * the card at `place.index` of the `place.of` in its row, drawn as the row's `look` says.
 */
export function EpisodeCard({
  show,
  episode,
  place,
  look,
}: {
  show: { libraryId: bigint; showId: ExternalId };
  episode: Episode;
  place: { index: number; of: number };
  look: RowLook;
}) {
  // Without its own ID there's nothing to mark or delete.
  const episodeId = episode.id;
  return (
    <li
      // The row's scroll reads its Season.
      data-season={episode.seasonNumber}
      aria-setsize={place.of}
      aria-posinset={place.index + 1}
      className={`shrink-0 snap-start ${backdropTile.widthClass}`}
      style={{ "--tile-width": backdropTile.width }}
    >
      <BackdropCard
        art={{ images: [stillOf(episode), look.backdrop], sizes: backdropTile.sizes }}
        link={{
          to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
          params: {
            libraryId: String(show.libraryId),
            showId: titleParam(show.showId),
            season: String(episode.seasonNumber),
            episode: String(episode.episodeNumber),
          },
        }}
        label={episodeCardLabel(episode)}
        tall={look.tall}
        caption={<EpisodeCaption episode={episode} />}
        menu={
          episodeId && (
            <EpisodeMoreLoader libraryId={show.libraryId} episodeId={episodeId} episode={episode} />
          )
        }
      />
    </li>
  );
}
