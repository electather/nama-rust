import { foundEpisodeStatus } from "@/features/title/lib/found-episode-status";
import type { RowEpisode } from "@/features/title/lib/show-row";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { episodeName, runtimeOf, timeLeft } from "@/shared/lib/title-facts";
import { isStarted } from "@/shared/lib/watch-progress";

/**
 * An Episode card's name, which says all its caption does: such as "Play Season 1, Episode 1:
 * Godflame, 57m", "Resume …, 31m left" once started, or "…, watched" once played, for one in the
 * page's Library; "Season 1, Episode 1: Godflame, Not on this Server" for one it lacks.
 */
export function episodeCardLabel(item: RowEpisode) {
  const what = `${episodeName(item.seasonNumber, item.episodeNumber, "full")}: ${item.episode.title}`;
  if (item.kind !== "here") {
    return [what, foundEpisodeStatus(item)].filter(Boolean).join(", ");
  }
  const { watchState, runtimeMinutes } = item.episode;
  const started = isStarted(watchState);
  return [
    `${started ? "Resume" : "Play"} ${what}`,
    started ? timeLeft(watchState) : runtimeOf(runtimeMinutes, "short"),
    watchedWord(watchState),
  ]
    .filter(Boolean)
    .join(", ");
}

/** Whether an Episode was played and not started again since, which its card shows as watched. */
export function playedNotStarted(state: WatchState | undefined) {
  return state?.played === true && !isStarted(state);
}

function watchedWord(state: WatchState | undefined) {
  return playedNotStarted(state) ? "watched" : undefined;
}
