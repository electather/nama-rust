import type { LinkOptions } from "@tanstack/react-router";

import type { Episode } from "@/gen/nama/v1/browse_pb";

/** The Episode that plays after this one, and how to open it. */
export interface NextEpisode {
  episode: Episode;
  /** How many Episodes in a row autoplay started with no input, ending with this one: 0 if it didn't start it. */
  streak: number;
  /** Its play route, opened with `streak`. */
  link: (streak: number) => LinkOptions;
}

/** An Episode player's search param `?streak=`: `NextEpisode.streak`, if autoplay opened it. */
export function streakSearch({ streak }: Record<string, unknown>): { streak?: number } {
  return typeof streak === "number" ? { streak } : {};
}

/** What the player autoplays next: `episode` of the Show `params` names, after one autoplay `streak` opened. */
export function nextEpisode(
  episode: Episode | undefined,
  streak: number | undefined,
  { libraryId, showId }: { libraryId: string; showId: string },
): NextEpisode | undefined {
  if (!episode) {
    return undefined;
  }
  return {
    episode,
    streak: streak ?? 0,
    link: (nextStreak) => ({
      ...playLink(episode, { libraryId, showId }),
      search: { streak: nextStreak },
    }),
  };
}

/**
 * The play route of `episode`, of the Show `params` names, in place of the Episode playing, so the
 * player stays one history entry, which Back leaves (`useGoBack`).
 */
export function playLink(
  { seasonNumber, episodeNumber }: Pick<Episode, "seasonNumber" | "episodeNumber">,
  { libraryId, showId }: { libraryId: string; showId: string },
) {
  return {
    to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
    params: { libraryId, showId, season: String(seasonNumber), episode: String(episodeNumber) },
    replace: true,
  } satisfies LinkOptions;
}

/** How long the countdown to the next Episode runs, in seconds. */
export const COUNTDOWN_SECONDS = 20;
/** How many Episodes in a row autoplay starts with no input before the player asks instead. */
const ASK_AFTER_STREAK = 3;

/**
 * When `next` plays by itself, in seconds: `COUNTDOWN_SECONDS` into the Credits, if `credits` says
 * when they start, or else at the end of the video.
 */
export function nextAt(duration: number, credits: number | undefined) {
  return credits === undefined ? duration : Math.min(credits + COUNTDOWN_SECONDS, duration);
}

/** `next` while the countdown to it runs: from the start of the `credits`, or near the end without them. */
export function dueNext(
  next: NextEpisode | undefined,
  { time, duration }: { time: number; duration: number },
  credits: number | undefined,
) {
  return duration > 0 && time >= (credits ?? duration - COUNTDOWN_SECONDS) ? next : undefined;
}

/** `countdown` once it has run out at `time`, if it does so in the Credits before the end, which `onEnded` can't see. */
export function endsEarly(
  countdown: NextEpisode | undefined,
  { time, duration }: { time: number; duration: number },
  at: number,
) {
  return at < duration && time >= at ? countdown : undefined;
}

/** The streak autoplay opens `next` with: input during this Episode starts the count again. */
export function streakAfter({ streak }: NextEpisode, touched: boolean) {
  return touched ? 1 : streak + 1;
}

/** Whether "Are you still watching?" is due before `due` plays. */
export function isStillWatchingDue(due: NextEpisode | undefined, touched: boolean) {
  return !touched && due !== undefined && due.streak >= ASK_AFTER_STREAK;
}
