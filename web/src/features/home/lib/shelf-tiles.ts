import type { RowTitle } from "@/shared/lib/row-titles";

/**
 * A shelf's tile: its width class, the width the virtualizer guesses before measuring, and its
 * corners' radius class.
 */
export interface ShelfTile {
  width: string;
  estimate: number;
  radius: string;
}

/** A 4:3 card, such as Continue Watching's, about three to a wide screen, with 18px corners. */
export const backdropTile: ShelfTile = {
  width: "w-tile-backdrop",
  estimate: 440,
  radius: "rounded-2xl",
};

/** A 2:3 poster, about seven to a wide screen. */
export const posterTile: ShelfTile = {
  width: "w-tile-poster",
  estimate: 210,
  radius: "rounded-3xl",
};

/**
 * A row's titles so far, as its shelf of posters shows them, and how it loads its next page, if
 * it has pages.
 */
export interface ShelfTitles {
  titles: RowTitle[];
  setSize: number | undefined;
  next?: { has: boolean; load: (options: { cancelRefetch: boolean }) => Promise<unknown> };
}

/**
 * A Recently Added row's one page: about 20 posters, a little over two wide screens' worth. The
 * rest are a click away on the Library page its heading opens.
 */
export const recentlyAddedPage = { size: 20 };

/** A page of a row: a little over a wide screen's shelf of posters. */
export const rowPage = { size: 12 };
