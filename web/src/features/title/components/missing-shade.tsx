import { EpisodeCorner } from "@/features/title/components/episode-corner";
import type { Standing } from "@/shared/lib/request-standing";

/**
 * What dims a missing Episode's Still, greyed and darkened toward the page, with where its Season
 * `season` is on its way, by the Episode's `standing`, in its top corner.
 */
export function MissingShade({ season, standing }: { season: number; standing: Standing }) {
  return (
    <>
      <div aria-hidden className="absolute inset-0 bg-background/55 backdrop-grayscale" />
      <EpisodeCorner season={season} standing={standing} />
    </>
  );
}
