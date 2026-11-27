import type { RowEpisode } from "@/features/title/lib/show-row";

/** A card's art, by its address and its placeholder's thumbhash. */
export interface CardArt {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
}

/**
 * How every card of a Show's row is drawn: square when any of its Episodes has a synopsis, so the
 * row keeps one height, and `backdrop`, the Show's, standing in for a missing Still.
 */
export interface RowLook {
  tall: boolean;
  backdrop: CardArt;
}

/** How the row of `episodes` draws its cards, with the Show's `backdrop`. */
export function rowLook(episodes: RowEpisode[], backdrop: CardArt): RowLook {
  return { tall: episodes.some(({ episode }) => episode.overview !== undefined), backdrop };
}

/** A card's art: its Episode's `still`, or the row's backdrop while the title source has none. */
export function cardArt(still: CardArt, look: RowLook): CardArt {
  return still.url === undefined ? look.backdrop : still;
}
