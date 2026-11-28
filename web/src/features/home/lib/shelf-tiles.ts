import type { RowTitle } from "@/shared/lib/row-titles";

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
