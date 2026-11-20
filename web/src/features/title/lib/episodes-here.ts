import { seasonsOf } from "@/features/title/lib/seasons-of";

/** How many Episodes a Show has here, and in how many Seasons, such as "19, in 2 Seasons". */
export function episodesHere(episodes: number, seasons: number) {
  const inSeasons = seasonsOf(seasons);
  return inSeasons === undefined ? String(episodes) : `${episodes}, in ${inSeasons}`;
}
