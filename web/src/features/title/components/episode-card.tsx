import { EpisodeCaption } from "@/features/title/components/episode-caption";
import { EpisodeCardMenu } from "@/features/title/components/episode-card-menu";
import { EpisodePlayLink } from "@/features/title/components/episode-play-link";
import { MissingShade } from "@/features/title/components/missing-shade";
import { episodeCardLabel } from "@/features/title/lib/episode-card-label";
import type { RowLook } from "@/features/title/lib/row-look";
import type { RowEpisode } from "@/features/title/lib/show-row";
import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";
import { stillOf } from "@/shared/lib/server-image";

/**
 * An Episode of a Show's row by its Still, with its caption over the Still, as Continue Watching's
 * cards are drawn. One in the page's Library plays its largest Version and has a "…", by
 * `libraryId`, to mark it watched or delete it; one in another Library plays nothing; one missing
 * from the Server is dimmed, with where its Season's Request is in its top corner. It is the card at
 * `place.index` of the `place.of` in its row, drawn as the row's `look` says.
 */
export function EpisodeCard({
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
  const label = episodeCardLabel(item);
  return (
    <li
      // The row's scroll reads its Season.
      data-season={item.seasonNumber}
      aria-setsize={place.of}
      aria-posinset={place.index + 1}
      className="w-tile-backdrop shrink-0 snap-start"
    >
      <BackdropFrame
        art={{ images: [stillOf(item.episode), look.backdrop], sizes: backdropTileSizes }}
        tall={look.tall}
        caption={<EpisodeCaption item={item} />}
        menu={<EpisodeCardMenu libraryId={libraryId} item={item} />}
      >
        {item.kind === "here" ? (
          <EpisodePlayLink episode={item.episode} label={label} />
        ) : (
          // The caption is hidden from screen readers, as a card's link names it, so this does.
          <span className="sr-only">{label}</span>
        )}
        {item.kind === "missing" && (
          <MissingShade season={item.seasonNumber} standing={item.standing} />
        )}
      </BackdropFrame>
    </li>
  );
}
