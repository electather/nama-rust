import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { TileArt } from "@/shared/components/tile-art";

/** The `sizes` of a shelf's poster, as wide as `w-tile-poster`. */
const shelfSizes = "(max-width: 1071px) 150px, (max-width: 1500px) 14vw, 210px";

const radius = "rounded-3xl";

/**
 * A Movie's, Show's or Box Set's poster alone, opening `link`: its name, `title`, and `detail`,
 * such as its year, are its label for assistive technology, and its name shows in place of art it
 * lacks. `children`, such as a badge, sit over it, and `describedBy` names the element that
 * describes the link, since its label hides them. `current` marks the title whose page it is on.
 * `sizes` is how wide it shows, a shelf's tile unless a grid says otherwise.
 */
export function Poster({
  link,
  title,
  detail,
  art,
  sizes = shelfSizes,
  current = false,
  describedBy,
  children,
}: {
  link: LinkOptions;
  title: string;
  detail?: string | undefined;
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
  sizes?: string;
  current?: boolean;
  describedBy?: string | undefined;
  children?: ReactNode;
}) {
  return (
    <Link
      {...link}
      aria-label={detail === undefined ? title : `${title}, ${detail}`}
      aria-current={current ? "page" : undefined}
      aria-describedby={describedBy}
      className={`relative isolate flex aspect-2/3 items-center justify-center overflow-hidden bg-muted p-3 outline-none focus-visible:ring-3 focus-visible:ring-ring/50 ${radius}`}
    >
      {art.url === undefined && (
        <span className="text-center text-sm font-medium text-balance text-muted-foreground">
          {title}
        </span>
      )}
      <TileArt
        url={art.url}
        thumbhash={art.thumbhash}
        look={{ sizes, radius, widths: "posterWidths" }}
      />
      {children}
    </Link>
  );
}
