import { seasonsOf } from "@/features/title/lib/seasons-of";

/**
 * How many Episodes a Show has here of the `aired` that have, and in how many Seasons, such as
 * "19 of 20, in 2 Seasons"; only how many are here while `aired` is 0, as when its source gave no
 * air dates.
 */
export function episodesHere(episodes: number, aired: number, seasons: number) {
  const ofAired = aired > 0 ? `${episodes} of ${aired}` : String(episodes);
  const inSeasons = seasonsOf(seasons);
  return inSeasons === undefined ? ofAired : `${ofAired}, in ${inSeasons}`;
}
