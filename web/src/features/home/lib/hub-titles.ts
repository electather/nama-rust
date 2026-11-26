import type { LinkOptions } from "@tanstack/react-router";

import type { Card } from "@/features/home/lib/continue-watching-cards";
import type { ReachedServer } from "@/features/home/lib/hub-rows";
import type { RowTitle } from "@/shared/lib/row-titles";
import { titleParam } from "@/shared/lib/title-param";

/** A row's title on the Server `server`, which `transport` reaches. */
export type TitleOn = ReachedServer & { title: RowTitle };

/** A Continue Watching card on the Server `server`, which `transport` reaches. */
export type CardOn = ReachedServer & { card: Card };

/** `link`, opening on the Linked Server `server`, as the Hub opens a Server's pages (ADR 0105). */
export function onLinkedServer(link: LinkOptions, server: ReachedServer["server"]): LinkOptions {
  return { ...link, search: { server: server.id } };
}

/** `card`, from the Server `on` names, opening there. */
export function cardOn({ server, transport }: ReachedServer, card: Card): CardOn {
  const page = { ...card.page, link: onLinkedServer(card.page.link, server) };
  return { server, transport, card: { ...card, link: onLinkedServer(card.link, server), page } };
}

/**
 * One row from each Server's titles: their first titles in turn, then their second, and so on,
 * each title once. A title on one Server and missing from another is listed as on the Server.
 */
export function mergeTitles(lists: TitleOn[][]) {
  const kept = new Map<string, TitleOn>();
  const longest = Math.max(0, ...lists.map((list) => list.length));
  for (let at = 0; at < longest; at += 1) {
    for (const next of lists.flatMap((list) => list[at] ?? [])) {
      const key = titleKey(next.title);
      const was = kept.get(key);
      if (was === undefined || (isMissing(was.title) && !isMissing(next.title))) {
        kept.set(key, next);
      }
    }
  }
  return [...kept.values()];
}

/** Which title it is across Servers: its External ID, and whether it's a Movie or a Show. */
function titleKey({ case: kind, value }: RowTitle) {
  const show = kind === "show" || kind === "missingShow";
  return `${show ? "show" : "movie"}/${titleParam(value.id)}`;
}

function isMissing({ case: kind }: RowTitle) {
  return kind === "missingMovie" || kind === "missingShow";
}

/**
 * Each Server's Continue Watching as one, most recent first, each Movie or Show once, from the
 * Server it was last watched on. Watch State syncs, so a title is often on several (ADR 0013).
 */
export function mergeContinueWatching(lists: CardOn[][]) {
  const latest = new Map<string, { on: CardOn; when: number }>();
  for (const watched of lists.flatMap((list) => whenWatched(list))) {
    const key = watched.on.card.titleId;
    if (watched.when > (latest.get(key)?.when ?? -1)) {
      latest.set(key, watched);
    }
  }
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts a copy, and the lib target has no toSorted
  return [...latest.values()].sort((a, b) => b.when - a.when).map(({ on }) => on);
}

/**
 * Each card of one Server's Continue Watching with when it was last watched. A Show's next Episode
 * has no Watch State, but the Server lists it by its Show's latest, so it takes the card above's
 * time, or, at the top, comes before every other.
 */
function whenWatched(list: CardOn[]) {
  let above = Number.POSITIVE_INFINITY;
  return list.map((on) => {
    const updatedAt = on.card.backdrop.watchState?.updatedAt;
    above = updatedAt === undefined || updatedAt === 0n ? above : Number(updatedAt);
    return { on, when: above };
  });
}
