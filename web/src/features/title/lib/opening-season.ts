import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * The Season a Show's row opens on: `asked`, if `seasons` has it, else the Season of the Episode
 * Play starts, `toPlay`, else the first.
 */
export function openingSeason(
  seasons: number[],
  asked: number | undefined,
  toPlay: Episode | undefined,
) {
  return seasons.find((number) => number === asked) ?? playSeason(seasons, toPlay);
}

/**
 * The Season the row is on, `season`, while the Show still has it; once it's deleted, the one the
 * row would open on.
 */
export function seasonShown(seasons: number[], season: number, toPlay: Episode | undefined) {
  return seasons.includes(season) ? season : openingSeason(seasons, undefined, toPlay);
}

function playSeason(seasons: number[], toPlay: Episode | undefined) {
  return toPlay?.seasonNumber ?? seasons[0] ?? 0;
}
