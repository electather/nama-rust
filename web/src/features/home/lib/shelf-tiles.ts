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
