import type { Page } from "@/features/action-bar/lib/pages";
import {
  type Frame,
  type Group,
  type Row,
  matching,
  section,
} from "@/features/action-bar/lib/rows";
import { shortcutGroups } from "@/features/action-bar/lib/shortcuts";
import type { Found, FoundPerson, FoundTitle } from "@/features/action-bar/lib/titles";
import { TitleStatus } from "@/gen/nama/v1/search_pb";
import { modKey } from "@/shared/lib/mod-key";

/** What the action bar lists from. */
export interface BarState {
  frame: Frame;
  query: string;
  pages: readonly Page[];
  recents: readonly FoundTitle[];
  /** What the last search to answer found, or `undefined` before any has. */
  found: Found | undefined;
  /** Whether the search failed. */
  failed: boolean;
}

/** What picking a row can do. */
export interface BarActions {
  push: (frame: Frame) => void;
  go: (page: Page) => void;
  open: (title: FoundTitle) => void;
  openPerson: (personId: string) => void;
  retry: () => void;
}

const badges = { movie: "Movie", show: "Show", episode: "Episode" } as const;

/** What a row of Recent says beside its label: its kind, as Recent mixes them. */
function kindBadge(title: FoundTitle): Pick<Row, "badge" | "request"> {
  return { badge: badges[title.kind] };
}

/**
 * What a search result says beside its label: nothing when Available, else its Request, or for a
 * Followed Show with no open Season Request, that it's Requested.
 */
function statusBadge({ libraryId, request, status }: FoundTitle): Pick<Row, "badge" | "request"> {
  if (libraryId !== undefined) {
    return {};
  }
  if (request !== undefined) {
    return { request };
  }
  return { badge: status === TitleStatus.REQUESTED ? "Requested" : "Request" };
}

function titleRows(
  titles: readonly FoundTitle[],
  open: BarActions["open"],
  badgeOf = statusBadge,
): Row[] {
  return titles.map((title) => ({
    id: title.id,
    label: title.label,
    hint: title.hint,
    icon: title.kind === "movie" ? "movie" : "show",
    poster: title.poster ?? null,
    still: title.still,
    ...badgeOf(title),
    action: "Open",
    run: () => {
      open(title);
    },
  }));
}

function personRows(people: readonly FoundPerson[], openPerson: BarActions["openPerson"]): Row[] {
  return people.map((person) => ({
    id: `person:${person.personId}`,
    label: person.name,
    hint: "",
    icon: "person",
    poster: person.headshot ?? null,
    action: "Open",
    run: () => {
      openPerson(person.personId);
    },
  }));
}

/** What the search found, of the kinds `frame` searches, or the way to try a failed search again. */
function results(
  { frame, found, failed }: BarState,
  { open, openPerson, retry }: BarActions,
): Group[] {
  if (failed) {
    const hint = "Couldn’t reach the Server. Check your connection and try again.";
    return [
      {
        heading: "Search failed",
        rows: [{ id: "retry", label: "Retry", hint, icon: "retry", run: retry }],
      },
    ];
  }
  if (found === undefined) {
    return [];
  }
  const movies = titleRows(found.movies, open);
  const shows = titleRows(found.shows, open);
  const episodes = section("Episodes", titleRows(found.episodes, open));
  return {
    root: [
      ...section("Movies", movies),
      ...section("Shows", shows),
      ...episodes,
      ...section("People", personRows(found.people, openPerson)),
    ],
    movie: section("Movie results", movies),
    show: [...section("Show results", shows), ...episodes],
    shortcuts: [],
  }[frame];
}

function rootGroups(state: BarState, actions: BarActions): Group[] {
  const blank = state.query.trim() === "";
  const scope = (frame: Frame, label: string, hint: string): Row => ({
    id: `search:${frame}`,
    label,
    hint,
    icon: frame === "movie" ? "movie" : "show",
    action: "Open",
    run: () => {
      actions.push(frame);
    },
  });
  const jumps = state.pages.map((page): Row => ({
    id: `page:${page.label}`,
    label: page.label,
    hint: page.hint,
    icon: page.icon,
    action: "Go",
    run: () => {
      actions.go(page);
    },
  }));
  const shortcuts: Row = {
    id: "action:shortcuts",
    label: "Keyboard shortcuts",
    hint: "View every active hotkey",
    icon: "keyboard",
    keys: [modKey, "/"],
    action: "Run",
    run: () => {
      actions.push("shortcuts");
    },
  };
  const search = [
    scope("show", "Search Shows", "Type to search the Shows in your Libraries"),
    scope("movie", "Search Movies", "Type to search the Movies in your Libraries"),
  ];
  return [
    ...section("Search", blank ? search : []),
    ...section("Recent", blank ? titleRows(state.recents, actions.open, kindBadge) : []),
    ...section("Jump to", matching(state.query, jumps)),
    ...(blank ? [] : results(state, actions)),
    ...section("Actions", matching(state.query, [shortcuts])),
  ];
}

/** The groups the action bar lists for where it is and what's typed, top to bottom. */
export function groupsOf(state: BarState, actions: BarActions): Group[] {
  if (state.frame === "root") {
    return rootGroups(state, actions);
  }
  if (state.frame === "shortcuts") {
    return shortcutGroups(state.query, state.pages);
  }
  if (state.query.trim() !== "") {
    return results(state, actions);
  }
  // ponytail: the Server has nothing like "trending", so before you type a search of one kind
  // lists what you opened lately of that kind. Give it a real list when the Server grows one.
  const kind = state.frame;
  const recent = state.recents.filter((title) => (title.kind === "movie") === (kind === "movie"));
  return section("Recent", titleRows(recent, actions.open, kindBadge));
}
