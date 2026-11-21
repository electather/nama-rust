import type { Season } from "@/gen/nama/v1/browse_pb";

/**
 * A Show's Seasons in the order its row of Episodes runs: numbered ones first, as the Server sorts
 * them, then Specials.
 */
export function rowSeasons(seasons: Season[]) {
  const numbers = seasons.map(({ number }) => number);
  return [...numbers.filter((number) => number > 0), ...numbers.filter((number) => number === 0)];
}
