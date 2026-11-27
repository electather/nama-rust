import type { RowEpisode } from "@/features/title/lib/show-row";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { type Standing, seasonStanding } from "@/shared/lib/request-standing";

/** What a Show's main button does: play an Episode, or Request its Season, by its `standing`. */
export type HeroTarget =
  | { kind: "play"; episode: Episode }
  | { kind: "ask"; season: number; standing: Standing };

/**
 * The next thing you'd watch of a Show's `row`: `resumed`, the Episode Continue Watching has, else
 * the first numbered Episode not played, which for one missing from the Server is Requesting its
 * Season, skipping those whose Season you can't act on: a Failed one only while you `mayRetry`. Once
 * every one is played, or there's only Specials, the first Episode on the Server.
 */
export function heroTarget(
  row: RowEpisode[],
  resumed: Episode | undefined,
  mayRetry: boolean,
): HeroTarget | undefined {
  if (resumed) {
    return { kind: "play", episode: resumed };
  }
  const next = row.find(
    (item) =>
      item.seasonNumber > 0 &&
      ((item.kind === "missing" && actsOn(item.standing, mayRetry)) ||
        (item.kind === "here" && item.episode.watchState?.played !== true)),
  );
  if (next?.kind === "missing") {
    return { kind: "ask", season: next.seasonNumber, standing: seasonStanding(next.standing) };
  }
  if (next?.kind === "here") {
    return { kind: "play", episode: next.episode };
  }
  const first = row.find((item) => item.kind === "here");
  return first?.kind === "here" ? { kind: "play", episode: first.episode } : undefined;
}

/** Whether the main button can act on a Season by its `standing`: a Failed one by Retry. */
function actsOn(standing: Standing, mayRetry: boolean) {
  return standing.can !== "nothing" || (mayRetry && seasonStanding(standing).kind === "stalled");
}

/** The Episode `target` plays, if it plays one. */
export function targetEpisode(target: HeroTarget | undefined) {
  return target?.kind === "play" ? target.episode : undefined;
}

/** The Season `target` is in, by which the row opens on it. */
export function targetSeason(target: HeroTarget | undefined) {
  return target?.kind === "ask" ? { seasonNumber: target.season } : target?.episode;
}

/** The Season `target` Requests, with where it stands, if it Requests one. */
export function targetAsk(target: HeroTarget | undefined) {
  return target?.kind === "ask" ? target : undefined;
}
