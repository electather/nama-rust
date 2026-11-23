import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { TileArt } from "@/shared/components/tile-art";

/** The `sizes` of a shelf's poster, as wide as `w-tile-poster`. */
const posterSizes = "(max-width: 1071px) 150px, (max-width: 1500px) 14vw, 210px";

const radius = "rounded-3xl";

/**
 * A title on a shelf: its poster alone, which carries its name, named `title` for assistive
 * technology and opening `link`, with `children`, such as a badge, over it. `describedBy` names
 * the element that describes the link, since its label hides the children. `current` marks the
 * title whose page the shelf is on.
 */
export function ShelfPoster({
  link,
  title,
  art,
  current = false,
  describedBy,
  children,
}: {
  link: LinkOptions;
  title: string;
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
  current?: boolean;
  describedBy?: string | undefined;
  children?: ReactNode;
}) {
  return (
    <Link
      {...link}
      aria-label={title}
      aria-current={current ? "page" : undefined}
      aria-describedby={describedBy}
      className={`relative isolate block aspect-2/3 overflow-hidden bg-muted outline-none focus-visible:ring-3 focus-visible:ring-ring/50 ${radius}`}
    >
      <TileArt url={art.url} thumbhash={art.thumbhash} look={{ sizes: posterSizes, radius }} />
      {children}
    </Link>
  );
}
