import type { InShow } from "@/features/player/lib/in-show";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/** A row of the Episodes panel's list: a Season's heading while `episode` is missing, or one of its Episodes. */
export interface EpisodeItem {
  season: number;
  episode: Episode | undefined;
}

/**
 * Every Episode of `seasons`, whose Episodes `episodes` holds in the same order, as one list. Each
 * Season starts with its heading when `headed`.
 */
export function episodeItems(seasons: number[], episodes: Episode[][], headed: boolean) {
  return seasons.flatMap((season, at): EpisodeItem[] => [
    ...(headed ? [{ season, episode: undefined }] : []),
    ...(episodes[at] ?? []).map((episode) => ({ season, episode })),
  ]);
}

/** Which Season the list reads as at `top`, the first row in view, unless one was `picked`. */
export function seasonShown(
  items: EpisodeItem[],
  top: number | undefined,
  picked: number | undefined,
) {
  return picked ?? items[top ?? 0]?.season;
}

/** Whether `item` is the Episode playing, as `inShow` says. */
export function playsNow(
  { season, episode }: EpisodeItem,
  inShow: Pick<InShow, "season" | "episodeNumber">,
) {
  return season === inShow.season.seasonNumber && episode?.episodeNumber === inShow.episodeNumber;
}
