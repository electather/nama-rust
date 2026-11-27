declare module "react" {
  interface CSSProperties {
    /** A tile's width, its shape's `width`, which its `widthClass` reads. */
    "--tile-width"?: string;
  }
}

// Every tile's shape is read from here (`src/AGENTS.md`, "One module knows each tile's shape").

/**
 * A tile shape: how wide a tile draws, its CSS `width`, which a tile's `--tile-width` style gives
 * its `widthClass`, at most `widest` pixels, a virtualizer's guess before it measures one; the
 * `sizes` that width asks for (ADR 0039); and its aspect's and corners' classes.
 */
export interface TileShape {
  width: string;
  widthClass: string;
  widest: number;
  sizes: string;
  aspect: string;
  radius: string;
}

/**
 * A tile at least `least` pixels wide, `vw` percent of the screen, at most `most` pixels: its
 * width and the `sizes` that ask for it, with the screens' widths where it stops growing.
 */
function widthOf(least: number, vw: number, most: number) {
  const screenFor = (px: number) => Math.floor((px * 100) / vw);
  return {
    width: `clamp(${least}px, ${vw}vw, ${most}px)`,
    // Tailwind sees only literal classes, so the width itself is a style.
    widthClass: "w-(--tile-width)",
    widest: most,
    sizes: `(max-width: ${screenFor(least)}px) ${least}px, (max-width: ${screenFor(most)}px) ${vw}vw, ${most}px`,
  };
}

/** A 2:3 Poster, about seven to a wide screen. */
export const posterTile: TileShape = {
  ...widthOf(150, 14, 210),
  aspect: "aspect-2/3",
  radius: "rounded-xl",
};

/** A 4:3 backdrop card, about three to a wide screen, square when it's `tall`. */
export const backdropTile: TileShape & { tallAspect: string } = {
  ...widthOf(280, 30, 440),
  aspect: "aspect-4/3",
  tallAspect: "aspect-square",
  radius: "rounded-xl",
};

/** The gap between a row's tiles, as a class and in pixels for a virtualizer's sums. */
export const tileGap = { className: "gap-5", px: 20 };
