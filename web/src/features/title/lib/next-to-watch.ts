import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * The Episode Play starts when nothing's in Continue Watching: the first not yet played, Specials
 * aside, or else the first.
 */
export function nextToWatch(episodes: Episode[]) {
  const numbered = episodes.filter(({ seasonNumber }) => seasonNumber > 0);
  return numbered.find(({ watchState }) => watchState?.played !== true) ?? episodes[0];
}
