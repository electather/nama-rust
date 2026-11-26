import { FoundEpisodeCaption } from "@/features/title/components/found-episode-caption";
import { MissingShade } from "@/features/title/components/missing-shade";
import { foundEpisodeLabel } from "@/features/title/lib/found-episode-label";
import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";

/**
 * An Episode from the title source that the page's Library doesn't have, by its Still, which plays
 * nothing: `elsewhere` in another Library, or `missing` from the Server, dimmed, with where its
 * Season's Request is in its top corner. It is the card at `place.index` of the `place.of` in its row.
 */
export function FoundEpisodeCard({
  item,
  place,
}: {
  item: FoundRowEpisode;
  place: { index: number; of: number };
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
          url: episode.stillUrl,
          thumbhash: undefined,
          sizes: backdropTileSizes,
          widths: "imageWidths",
        }}
        tall={episode.overview !== undefined}
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
