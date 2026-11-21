import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { episodeName, runtimeOf, timeLeft } from "@/shared/lib/title-facts";
import { isStarted } from "@/shared/lib/watch-progress";

/**
 * An Episode card's name, which says all its caption does: such as "Play Season 1, Episode 1:
 * Godflame, 57m", "Resume …, 31m left" once started, or "…, watched" once played.
 */
export function episodeCardLabel(episode: Episode) {
  const { seasonNumber, episodeNumber, title, watchState } = episode;
  const what = `${episodeName(seasonNumber, episodeNumber, "full")}: ${title}`;
  const started = isStarted(watchState);
  return [
    `${started ? "Resume" : "Play"} ${what}`,
    started ? timeLeft(watchState) : runtimeOf(episode.runtimeMinutes, "short"),
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
