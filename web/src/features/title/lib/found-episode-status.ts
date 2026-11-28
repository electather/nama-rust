import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { calendarDay, runtimeOf } from "@/shared/lib/title-facts";

/**
 * A found Episode card's last line: its runtime when another Library has it, when it airs while it
 * hasn't yet, else where its Season's Request is, such as "Pending approval", or "Not on this
 * Server" while it has none.
 */
export function foundEpisodeStatus(item: FoundRowEpisode) {
  if (item.kind === "elsewhere") {
    return runtimeOf(item.episode.runtimeMinutes, "short");
  }
  const { standing } = item;
  if (standing.kind === "notAired") {
    return `Airs ${calendarDay(standing.airDate)}`;
  }
  return "request" in standing ? shownOnTitle(standing.request.state).label : "Not on this Server";
}
