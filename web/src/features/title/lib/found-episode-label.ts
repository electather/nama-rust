import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { calendarDay, episodeName, isLaterDay, runtimeOf } from "@/shared/lib/title-facts";

/**
 * A found Episode card's last line: its runtime when another Library has it, when it airs while it
 * hasn't yet, else where its Season's Request is, such as "Pending approval", or "Not on this
 * Server" while it has none.
 */
export function foundEpisodeStatus(item: FoundRowEpisode) {
  if (item.kind === "elsewhere") {
    return runtimeOf(item.episode.runtimeMinutes, "short");
  }
  const { airDate } = item.episode;
  if (airDate !== undefined && isLaterDay(airDate)) {
    return `Airs ${calendarDay(airDate)}`;
  }
  return item.request ? shownOnTitle(item.request.state).label : "Not on this Server";
}

/** A found Episode card's name, which says all its caption does. */
export function foundEpisodeLabel(item: FoundRowEpisode) {
  const what = `${episodeName(item.seasonNumber, item.episodeNumber, "full")}: ${item.episode.title}`;
  return [what, foundEpisodeStatus(item)].filter(Boolean).join(", ");
}
