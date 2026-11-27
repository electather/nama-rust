import { FoundEpisodeCaption } from "@/features/title/components/found-episode-caption";
import { MissingShade } from "@/features/title/components/missing-shade";
import { foundEpisodeLabel } from "@/features/title/lib/found-episode-label";
import { type RowLook, cardArt } from "@/features/title/lib/row-look";
import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";

/**
 * An Episode from the title source that the page's Library doesn't have, by its Still, which plays
 * nothing: `elsewhere` in another Library, or `missing` from the Server, dimmed, with where its
 * Season's Request is in its top corner. It is the card at `place.index` of the `place.of` in its row,
 * drawn as the row's `look` says.
 */
export function FoundEpisodeCard({
  item,
  place,
  look,
}: {
  item: FoundRowEpisode;
  place: { index: number; of: number };
  look: RowLook;
}) {
  const { episode, seasonNumber } = item;
  return (
    <li
      // The row's scroll reads its Season.
      data-season={seasonNumber}
      aria-setsize={place.of}
      aria-posinset={place.index + 1}
      className="w-tile-backdrop shrink-0 snap-start"
    >
      <BackdropFrame
        art={{
          ...cardArt({ url: episode.stillUrl, thumbhash: undefined }, look),
          sizes: backdropTileSizes,
          widths: "imageWidths",
        }}
        tall={look.tall}
        caption={<FoundEpisodeCaption item={item} />}
        menu={null}
      >
        {/* The caption is hidden from screen readers, as a card's link names it, so this does. */}
        <span className="sr-only">{foundEpisodeLabel(item)}</span>
        {item.kind === "missing" && (
          <MissingShade season={seasonNumber} request={item.request} askable={item.askable} />
        )}
      </BackdropFrame>
    </li>
  );
}
