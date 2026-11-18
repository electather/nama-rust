import type { Season } from "@/gen/nama/v1/browse_pb";

/** A Show's one Season that isn't Specials, if it has exactly one. */
function onlySeason(seasons: Season[]) {
  const numbered = seasons.filter(({ number }) => number > 0);
  return numbered.length === 1 ? numbered[0] : undefined;
}

/**
 * A Season the Show doesn't have, or none asked for, means its first. Specials (0) come first only
 * when they're all there is, so a one-Season Show opens on that Season with the Specials below.
 */
export function seasonToShow(seasons: Season[], asked: number | undefined) {
  return (
    seasons.find(({ number }) => number === asked) ??
    seasons.find(({ number }) => number > 0) ??
    seasons[0]
  );
}

/**
 * Whether the page lists the Show's only Season, without Season links. Asks of the Season `shown`,
 * as Events change an open page's Seasons but not the one it shows.
 */
export function listsOnlySeason(seasons: Season[], shown: number) {
  return onlySeason(seasons)?.number === shown;
}

/** Whether the Specials are listed below the Season `shown`, a Show's only one. */
export function specialsBelow(seasons: Season[], shown: number) {
  return listsOnlySeason(seasons, shown) && seasons.some(({ number }) => number === 0);
}
