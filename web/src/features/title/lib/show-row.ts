import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { FoundEpisode, FoundShow, Request } from "@/gen/nama/v1/request_pb";

/**
 * One card of a Show's row: an Episode `here`, in the page's Library, or one from the title source,
 * `elsewhere` in another Library or `missing` from the Server, with its Season's `request`. A
 * missing one is `askable` unless its Season is in a Library already, as one not aired yet is.
 */
export type RowEpisode = { seasonNumber: number; episodeNumber: number } & (
  | { kind: "here"; episode: Episode }
  | { kind: "elsewhere"; episode: FoundEpisode }
  | { kind: "missing"; episode: FoundEpisode; request: Request | undefined; askable: boolean }
);

/** A card of a Show's row for an Episode the page's Library doesn't have. */
export type FoundRowEpisode = Extract<RowEpisode, { kind: "elsewhere" | "missing" }>;

/**
 * Every Episode of a Show in the order its row runs, numbered Seasons first, then Specials: the
 * Library's `episodes`, and those of `found`, the Show from the title source, the Library lacks.
 */
export function showRow(episodes: Episode[], found?: FoundShow): RowEpisode[] {
  const has = new Set(
    episodes.map((episode) => place(episode.seasonNumber, episode.episodeNumber)),
  );
  const row: RowEpisode[] = episodes.map((episode) => ({
    kind: "here",
    seasonNumber: episode.seasonNumber,
    episodeNumber: episode.episodeNumber,
    episode,
  }));
  for (const { seasonNumber, request, inLibrary, episodes: foundEpisodes } of found?.seasons ??
    []) {
    for (const episode of foundEpisodes) {
      const { episodeNumber } = episode;
      if (!has.has(place(seasonNumber, episodeNumber))) {
        const at = { seasonNumber, episodeNumber, episode };
        row.push(
          episode.inLibrary
            ? { kind: "elsewhere", ...at }
            : { kind: "missing", ...at, request, askable: !inLibrary },
        );
      }
    }
  }
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
  return row.sort(byPlace);
}

function place(season: number, episode: number) {
  return `${season}x${episode}`;
}

/** Specials, Season 0, sort after every numbered Season. */
function seasonOrder(number: number) {
  return number === 0 ? Number.MAX_SAFE_INTEGER : number;
}

function byPlace(a: RowEpisode, b: RowEpisode) {
  const bySeason = seasonOrder(a.seasonNumber) - seasonOrder(b.seasonNumber);
  return bySeason === 0 ? a.episodeNumber - b.episodeNumber : bySeason;
}

/**
 * The Seasons of a Show's `row`, with any of `also`, such as a Library's Season with no Episode
 * listed yet, in the row's order.
 */
export function seasonsOfRow(row: RowEpisode[], also: number[] = []) {
  const seasons = [...new Set([...also, ...row.map(({ seasonNumber }) => seasonNumber)])];
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
  return seasons.sort((a, b) => seasonOrder(a) - seasonOrder(b));
}
