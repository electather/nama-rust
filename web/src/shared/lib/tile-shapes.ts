declare module "react" {
  interface CSSProperties {
    /** A tile's width, its shape's `width`, which its `widthClass` reads. */
    "--tile-width"?: string;
  }
}

// Every tile's shape is read from here (`src/AGENTS.md`, "One module knows each tile's shape").

/** A tile's shape: how wide it draws, the `sizes` that asks for (ADR 0039), its aspect and corners. */
export interface TileShape {
  /** The CSS width, given as a tile's `--tile-width` style for its `widthClass` to read. */
  width: string;
  widthClass: string;
  /** The widest it draws, in pixels: a virtualizer's guess before it measures one. */
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
    // Tailwind sees only literal classes, so the width is a style, and a custom property, the only
    // inline style `shadcn/no-inline-styles` allows.
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

/** The `li` of a row's `tile`: its width, which its shape's `widthClass` reads, and snapping. */
export function tileItem(tile: TileShape) {
  return {
    className: `shrink-0 snap-start ${tile.widthClass}`,
    style: { "--tile-width": tile.width },
  };
}

/**
 * The classes of a grid of posters: 2 columns, 3 from `sm`, 4 from `md` and 5 from `lg`. A Library's
 * grid fits columns to its width instead, in `useGridColumns`.
 */
export const posterGrid =
  "mt-8 grid grid-cols-2 gap-x-4 gap-y-8 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5";

/**
 * The hairline inside a tile's edge, drawn over its art by a span of the tile's radius. It fades
 * out as the tile lifts (`tileMotion`), which is the `group/tile` it reads.
 */
export const tileRing =
  "absolute inset-0 inset-ring inset-ring-border transition-opacity duration-300 ease-out-expo group-hover/tile:opacity-0 group-focus-visible/tile:opacity-0 group-has-focus-visible/tile:opacity-0 motion-reduce:duration-0";

/**
 * How a linked tile, a Poster or backdrop card, moves (DESIGN.md, "Card motion"): it lifts on hover
 * and keyboard focus, sinks on press, and leans toward the pointer as a sheen follows it
 * (`tile-tilt`, `tile-sheen`, `followPointer`). Only `scale`, `transform`
 * and `box-shadow` change, never a size or margin, so no neighbour
 * moves; `z-1` lets the lifted tile overlap them, and a row's room for the lift is `shelfBleed`.
 * Under reduced motion nothing scales and the shadow changes at once. The tile's ring and art
 * read its `group/tile`.
 */
export const tileMotion = [
  "group/tile tile-sheen tile-tilt",
  "transition-lift motion-reduce:duration-0",
  "hover:z-1 hover:shadow-lg focus-visible:z-1 focus-visible:shadow-lg has-focus-visible:z-1 has-focus-visible:shadow-lg",
  "motion-safe:hover:scale-102 motion-safe:focus-visible:scale-102 motion-safe:has-focus-visible:scale-102",
  "motion-safe:active:scale-97 motion-safe:active:duration-100",
].join(" ");

/** How a poster's box sits and loads: its extra classes, `sizes`, corners, and whether it names its title. */
export interface PosterSize {
  /** The box's own size classes: none where its parent sets its width. */
  box: string;
  sizes: string;
  radius: string;
  /** Whether a poster without art shows its title; a thumbnail is an empty box. */
  named: boolean;
}

/** Each size a poster draws at, all in `posterTile`'s 2:3 aspect. */
export const posterSizes = {
  /** On a row: as wide as its `posterTile` item. */
  shelf: { box: "", sizes: posterTile.sizes, radius: posterTile.radius, named: true },
  /** In a grid, a Library's, a Watchlist's or a Person's: as wide as its column. */
  tile: {
    box: "",
    sizes: "(min-width: 1440px) 220px, (min-width: 768px) 20vw, 45vw",
    radius: posterTile.radius,
    named: true,
  },
  /** Leading a row of text, 56px tall. */
  thumb: { box: "h-14 shrink-0", sizes: "38px", radius: "rounded-xs", named: false },
} satisfies Record<string, PosterSize>;
