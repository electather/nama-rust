import { Link, type LinkOptions } from "@tanstack/react-router";
import { useId } from "react";

import { PosterArt } from "@/shared/components/poster/poster-art";
import { TileBadge, type TileBadgeShown } from "@/shared/components/poster/tile-badge";
import { followPointer } from "@/shared/lib/follow-pointer";
import type { ServerImage } from "@/shared/lib/server-image";
import { posterSizes, tileMotion } from "@/shared/lib/tile-shapes";

/** A title as a poster shows it: its art, its link, and what labels it for assistive technology. */
export interface PosterData {
  link: LinkOptions;
  title: string;
  /** What more labels it, such as its year. */
  detail?: string | undefined;
  art: ServerImage | undefined;
  /** Where a title missing from the Server stands, over its art's corner (DESIGN.md, "Posters"). */
  badge?: TileBadgeShown | undefined;
}

/**
 * A Movie's, Show's or Box Set's poster, opening its `link`: its name and `detail` are its label
 * for assistive technology, which the `badge`, if any, adds to. `current` marks the title whose
 * page it is on. `size` is how it draws, a shelf's tile unless given.
 */
export function Poster({
  link,
  title,
  detail,
  art,
  badge,
  size = "shelf",
  current = false,
}: PosterData & { size?: keyof typeof posterSizes; current?: boolean }) {
  const badgeId = useId();
  return (
    <Link
      {...link}
      aria-label={detail === undefined ? title : `${title}, ${detail}`}
      aria-current={current ? "page" : undefined}
      aria-describedby={badge && badgeId}
      onPointerMove={followPointer}
      className={`relative block outline-none focus-visible:ring-3 focus-visible:ring-ring/50 ${tileMotion} ${posterSizes[size].radius}`}
    >
      <PosterArt art={art} title={title} size={size} />
      {badge && <TileBadge id={badgeId} badge={badge} />}
    </Link>
  );
}
