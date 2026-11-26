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

/** `card`, from the Server `server` that `transport` reaches, opening there. */
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
 * A title on Continue Watching at the Hub: its card from the Server it was last watched on, and
 * its card on every Server that lists it, `copies`, which its menu acts on.
 */
export type MergedCard = CardOn & { copies: CardOn[] };

/**
 * Each Server's Continue Watching as one, most recent first, each Movie or Show once, from the
 * Server it was last watched on. Watch State syncs, so a title is often on several (ADR 0013).
 */
export function mergeContinueWatching(lists: CardOn[][]): MergedCard[] {
  const latest = new Map<string, { on: CardOn; when: number; copies: CardOn[] }>();
  for (const { on, when } of lists.flatMap((list) => whenWatched(list))) {
    const key = on.card.titleId;
    const kept = latest.get(key);
    if (kept === undefined) {
      latest.set(key, { on, when, copies: [on] });
    } else {
      kept.copies.push(on);
      if (when > kept.when) {
        Object.assign(kept, { on, when });
      }
    }
  }
  const merged = [...latest.values()];
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts a copy, and the lib target has no toSorted
  merged.sort((a, b) => b.when - a.when);
  return merged.map(({ on: { server, transport, card }, copies }) => ({
    server,
    transport,
    card,
    copies,
  }));
}

/**
 * Each card of one Server's Continue Watching with when it was last watched. A Show's next Episode
 * has no Watch State: the Server lists it by its Show's latest, which is no older than the card
 * below's, so it takes that time, or none at the bottom, and a time known elsewhere beats it.
 */
function whenWatched(list: CardOn[]) {
  const watched: { on: CardOn; when: number }[] = [];
  let below = 0;
  for (let at = list.length - 1; at >= 0; at -= 1) {
    const on = list[at];
    const updatedAt = on?.card.backdrop.watchState?.updatedAt;
    below = updatedAt === undefined || updatedAt === 0n ? below : Number(updatedAt);
    if (on !== undefined) {
      watched.unshift({ on, when: below });
    }
  }
  return watched;
}
