import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { CardCaption } from "@/shared/components/backdrop-card/card-caption";
import { TileArt } from "@/shared/components/tile-art";

/** A card's corners, which its art's hairline and its link's focus ring follow. */
const radius = "rounded-2xl";

/**
 * A card of a title's or Episode's art that opens `link`, named `label`, such as a Continue
 * Watching card or an Episode's, with `caption` and `menu`, its "…", at its bottom. It's as wide
 * as its shelf's tile, 4:3, or square when `tall`, for a caption with a description, so the art
 * still shows above it.
 */
export function BackdropCard({
  art,
  link,
  label,
  tall = false,
  caption,
  menu,
}: {
  art: { url: string | undefined; thumbhash: Uint8Array | undefined; sizes: string };
  link: LinkOptions;
  label: string;
  tall?: boolean;
  caption: ReactNode;
  menu: ReactNode;
}) {
  return (
    <div
      className={`relative isolate overflow-hidden bg-muted ${tall ? "aspect-square" : "aspect-4/3"} ${radius}`}
    >
      <TileArt url={art.url} thumbhash={art.thumbhash} look={{ sizes: art.sizes, radius }} />
      <Link
        {...link}
        aria-label={label}
        className={`absolute inset-0 outline-none focus-visible:outline-2 focus-visible:-outline-offset-3 focus-visible:outline-ring ${radius}`}
      />
      <CardCaption caption={caption} menu={menu} />
    </div>
  );
}
