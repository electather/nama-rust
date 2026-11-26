import type { RowEpisode } from "@/features/title/lib/show-row";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { Request } from "@/gen/nama/v1/request_pb";

/** What a Show's main button does: play an Episode, or Request its Season, with its `request`. */
export type HeroTarget =
  | { kind: "play"; episode: Episode }
  | { kind: "ask"; season: number; request: Request | undefined };

/**
 * The next thing you'd watch of a Show's `row`: `resumed`, the Episode Continue Watching has, else
 * the first numbered Episode not played, which for one missing from the Server is Requesting its
 * Season, skipping those whose Season can't be asked for. Once every one is played, or there's only Specials, the first Episode on the Server.
 */
export function heroTarget(row: RowEpisode[], resumed?: Episode): HeroTarget | undefined {
  if (resumed) {
    return { kind: "play", episode: resumed };
  }
  const next = row.find(
    (item) =>
      item.seasonNumber > 0 &&
      ((item.kind === "missing" && item.askable) ||
        (item.kind === "here" && item.episode.watchState?.played !== true)),
  );
  if (next?.kind === "missing") {
    return { kind: "ask", season: next.seasonNumber, request: next.request };
  }
  if (next?.kind === "here") {
    return { kind: "play", episode: next.episode };
  }
  const first = row.find((item) => item.kind === "here");
  return first?.kind === "here" ? { kind: "play", episode: first.episode } : undefined;
}

/** The Episode `target` plays, if it plays one. */
export function targetEpisode(target: HeroTarget | undefined) {
  return target?.kind === "play" ? target.episode : undefined;
}

/** The Season `target` is in, by which the row opens on it. */
export function targetSeason(target: HeroTarget | undefined) {
  return target?.kind === "ask" ? { seasonNumber: target.season } : target?.episode;
}

/** The Season `target` Requests, with its Request, if it Requests one. */
export function targetAsk(target: HeroTarget | undefined) {
  return target?.kind === "ask" ? target : undefined;
}
