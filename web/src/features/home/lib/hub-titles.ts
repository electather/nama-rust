import type { LinkOptions } from "@tanstack/react-router";

import type { Card } from "@/features/home/lib/continue-watching-cards";
import type { Panel, PanelOn } from "@/features/home/lib/featured-panels";
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
  for (const next of inTurn(lists)) {
    const key = titleKey(next.title);
    const was = kept.get(key);
    if (was === undefined || (isMissing(was.title) && !isMissing(next.title))) {
      kept.set(key, next);
    }
  }
  return [...kept.values()];
}

/** As many Featured panels as one Server's Featured has at most (`ListFeatured`). */
const featuredCount = 5;

/** `panel`, from the Server `server` that `transport` reaches, opening there. */
export function panelOn({ server, transport }: ReachedServer, panel: Panel): PanelOn {
  const action = { ...panel.action, link: onLinkedServer(panel.action.link, server) };
  const page = onLinkedServer(panel.page, server);
  return { transport, panel: { ...panel, key: `${server.id}/${panel.key}`, action, page } };
}

/**
 * Each Server's Featured as one: their first panels in turn, then their second, and so on, each
 * title once, from the first Server to list it, and no more than one Server's Featured has.
 */
export function mergeFeatured(lists: PanelOn[][]) {
  const kept = new Map<string, PanelOn>();
  for (const next of inTurn(lists)) {
    const { kind, id } = next.panel.watchlist;
    const key = `${kind}/${titleParam(id)}`;
    if (!kept.has(key)) {
      kept.set(key, next);
    }
  }
  return [...kept.values()].slice(0, featuredCount);
}

/** Each list's first item in turn, then each one's second, and so on. */
function inTurn<Item>(lists: Item[][]) {
  const longest = Math.max(0, ...lists.map((list) => list.length));
  return Array.from({ length: longest }, (_, at) => lists.flatMap((list) => list[at] ?? [])).flat();
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
 * A card older than the last so far of a Server with `more` waits: its next page may come first.
 */
export function mergeContinueWatching(servers: { cards: CardOn[]; more: boolean }[]): MergedCard[] {
  const timed = servers.map(({ cards, more }) => ({ more, watched: whenWatched(cards, more) }));
  const shownSince = Math.max(
    0,
    ...timed
      .flatMap(({ more, watched }) => (more ? watched.slice(-1) : []))
      .map(({ when }) => when),
  );
  const latest = new Map<string, { on: CardOn; when: number; copies: CardOn[] }>();
  for (const { on, when } of timed.flatMap(({ watched }) => watched)) {
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
  const merged = [...latest.values()].filter(({ when }) => when >= shownSince);
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
 * below's, so it takes that time, or none at the bottom, and a time known elsewhere beats it. At
 * the bottom of a Server with `more`, the card below is still to come, so it's left out till then.
 */
function whenWatched(cards: CardOn[], more: boolean): { on: CardOn; when: number }[] {
  const watched: { on: CardOn; when: number }[] = [];
  let below = more ? undefined : 0;
  // oxlint-disable-next-line unicorn/no-array-reverse -- reverses a copy, and the lib target has no toReversed
  for (const on of [...cards].reverse()) {
    const updatedAt = on.card.backdrop.watchState?.updatedAt ?? 0n;
    below = updatedAt === 0n ? below : Number(updatedAt);
    if (below !== undefined) {
      watched.unshift({ on, when: below });
    }
  }
  // With no time on any card yet, none would ever show: list them as at the bottom.
  return more && watched.length === 0 ? whenWatched(cards, false) : watched;
}
