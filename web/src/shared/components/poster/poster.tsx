import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";
import { posterTile } from "@/shared/lib/tile-shapes";

/**
 * A Movie's, Show's or Box Set's poster alone, opening `link`: its name, `title`, and `detail`,
 * such as its year, are its label for assistive technology, and its name shows in place of art it
 * lacks or that fails to load. `children`, such as a badge, sit over it, and `describedBy` names the element that
 * describes the link, since its label hides them. `current` marks the title whose page it is on.
 * `sizes` is how wide it shows, a shelf's tile unless a grid says otherwise.
 */
export function Poster({
  link,
  title,
  detail,
  art,
  sizes = posterTile.sizes,
  current = false,
  describedBy,
  children,
}: {
  link: LinkOptions;
  title: string;
  detail?: string | undefined;
  art: ServerImage | undefined;
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
      className={`relative isolate flex items-center justify-center overflow-hidden bg-muted p-3 outline-none focus-visible:ring-3 focus-visible:ring-ring/50 ${posterTile.aspect} ${posterTile.radius}`}
    >
      <ServerImg
        images={[art]}
        sizes={sizes}
        className="absolute inset-0 size-full object-cover"
        fallback={
          <span className="text-center text-sm font-medium text-balance text-muted-foreground">
            {title}
          </span>
        }
      />
      <span
        aria-hidden
        className={`absolute inset-0 inset-ring inset-ring-border ${posterTile.radius}`}
      />
      {children}
    </Link>
  );
}
