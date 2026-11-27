import type { LinkOptions } from "@tanstack/react-router";

import { Poster } from "@/shared/components/poster/poster";
import type { ServerImage } from "@/shared/lib/server-image";

/** The `sizes` of a poster in a Library's grid: columns of at least 160px, at most 1,400px wide. */
const gridSizes = "(min-width: 1440px) 220px, (min-width: 768px) 20vw, 45vw";

/** A title in a Library's grid: its poster alone (DESIGN.md, "Posters"). */
export function PosterTile(props: {
  link: LinkOptions;
  title: string;
  detail?: string | undefined;
  art: ServerImage | undefined;
}) {
  return <Poster {...props} sizes={gridSizes} />;
}
