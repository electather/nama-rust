// The rule this module keeps is in `web/src/AGENTS.md` ("One module knows Settings' address"). The
// signed-in route parses with it, so it's in the first page's code: no generated code, no
// Permissions, no React.

/** Each Section of Settings, by the word `?settings=` opens it with. */
export const sections = [
  "signin",
  "devices",
  "playback",
  "home",
  "requests",
  "connections",
  "libraries",
  "needs-a-match",
  "now-playing",
  "jobs",
  "server",
  "users",
  "backups",
  "default-home",
  "server-playback",
  "plugins",
  "account",
  "account-signin",
  "account-sessions",
  "account-servers",
  "account-history",
] as const;

/** A Section of Settings. */
export type Section = (typeof sections)[number];

/** The Section Settings opens on when asked for one it doesn't have. */
export const firstSection: Section = "playback";

/** What `?settings=all` opens: on a phone the list of Sections, wider the first one beside it. */
export const allSections = "all";

/** What Settings opens on: one Section, or all of them. */
export type SettingsPage = Section | typeof allSections;

/**
 * What a Section opens in place of its list: a Plugin, a User, a Job or a Library (`new` for the
 * form that adds one), or in Needs a Match, the Library whose Items it filters to.
 */
export type SettingsDetail =
  | { kind: "plugin" | "library" | "library-filter"; id: string }
  | { kind: "user"; id: number }
  | { kind: "job"; pluginId: string; jobId: string };

/** The page's search, as the router reads it. */
type Search = Readonly<Record<string, unknown>>;

/** Where Settings is: closed (no `page`), or open on a page, with the Server picked at the Hub. */
export interface SettingsAddress {
  page: SettingsPage | undefined;
  /** What the page's Section opens; none for all of them. */
  detail: SettingsDetail | undefined;
  /** The setting a search found, to land on. */
  setting: string | undefined;
  server: string | undefined;
}

/** The page's search words that say where Settings is. */
export interface SettingsSearch {
  settings?: SettingsPage | undefined;
  manage?: string | undefined;
  plugin?: string | undefined;
  user?: number | undefined;
  job?: string | undefined;
  library?: string | undefined;
  setting?: string | undefined;
}

const noDetail = {
  plugin: undefined,
  user: undefined,
  job: undefined,
  library: undefined,
  setting: undefined,
  // Every word but the page's and the picked Server's, so a builder clears a new detail too.
} satisfies Record<Exclude<keyof SettingsSearch, "settings" | "manage">, undefined>;

/** Settings closed: every word emptied, the picked Server's too. */
export const closedSearch = { settings: undefined, manage: undefined, ...noDetail };

/**
 * Where `search`, the page's search, says Settings is. A word it can't read, a detail its page's
 * Section doesn't open, and any word while Settings is closed, are left out.
 */
export function parseSettings(search: Readonly<Record<string, unknown>>): SettingsAddress {
  const page = pageParam(search["settings"]);
  if (page === undefined) {
    return { page, detail: undefined, setting: undefined, server: undefined };
  }
  return {
    page,
    detail: page === allSections ? undefined : detailParams[page]?.(search),
    setting: idParam(search["setting"]),
    server: idParam(search["manage"]),
  };
}

/** The signed-in route's search: Settings' words as `parseSettings` reads them. */
export function validateSettingsSearch(search: Readonly<Record<string, unknown>>): SettingsSearch {
  const { page, detail, setting, server } = parseSettings(search);
  const words = detail && detailWords(detail);
  return { ...closedSearch, ...words, settings: page, setting, manage: server };
}

/** Settings on `page`, at its `setting` if asked for, with no detail. The picked Server stays. */
export function sectionSearch(page: SettingsPage, setting?: string) {
  return { ...noDetail, settings: page, setting };
}

/** Settings open on `detail` in its Section, with no other detail. The picked Server stays. */
export function detailSearch(detail: SettingsDetail) {
  return { ...noDetail, ...detailWords(detail) };
}

/** At the Hub, Settings of the Server `id`, or the Account's without one, on the Section open. */
export function pickedServerSearch(id: string | undefined) {
  return { ...noDetail, manage: id };
}

/** The id `detail` opens if it's of `kind`: none for another kind, or none open. */
export function openedId(
  detail: SettingsDetail | undefined,
  kind: Extract<SettingsDetail, { id: string }>["kind"],
) {
  return detail?.kind === kind ? detail.id : undefined;
}

/** An id the address holds, such as a Plugin's or a Server's: none for a value that isn't one. */
export function idParam(value: unknown) {
  // The router reads an id made of digits as a number.
  return typeof value === "string" || typeof value === "number" ? String(value) : undefined;
}

/** The words that open `detail` in its Section. */
function detailWords(detail: SettingsDetail): SettingsSearch {
  if (detail.kind === "user") {
    return { settings: "users", user: detail.id };
  }
  if (detail.kind === "job") {
    // Plugin and Job ids hold only a-z, 0-9 and `-`, so the colon names one Job.
    return { settings: "jobs", job: `${detail.pluginId}:${detail.jobId}` };
  }
  if (detail.kind === "plugin") {
    return { settings: "plugins", plugin: detail.id };
  }
  return {
    settings: detail.kind === "library" ? "libraries" : "needs-a-match",
    library: detail.id,
  };
}

/** How each Section that opens a detail reads it from the page's search. */
const detailParams: Partial<Record<Section, (search: Search) => SettingsDetail | undefined>> = {
  plugins: (search) => withId(idParam(search["plugin"]), (id) => ({ kind: "plugin", id })),
  users: (search) => withId(userParam(search["user"]), (id) => ({ kind: "user", id })),
  jobs: (search) => jobParam(search["job"]),
  libraries: (search) => withId(idParam(search["library"]), (id) => ({ kind: "library", id })),
  "needs-a-match": (search) =>
    withId(idParam(search["library"]), (id) => ({ kind: "library-filter", id })),
};

function withId<T>(id: T | undefined, open: (id: T) => SettingsDetail) {
  return id === undefined ? undefined : open(id);
}

const JOB_KEY = /^(?<pluginId>[^:]+):(?<jobId>[^:]+)$/u;

/** The Job `?job=` names by its Plugin's id and its own. */
function jobParam(value: unknown) {
  const { pluginId, jobId } = JOB_KEY.exec(idParam(value) ?? "")?.groups ?? {};
  return pluginId === undefined || jobId === undefined
    ? undefined
    : { kind: "job" as const, pluginId, jobId };
}

/** The User `?user=` names: a whole number. */
function userParam(value: unknown) {
  // The router writes a string of digits quoted (`user=%222%22`), and `Number` reads a blank as 0.
  const id = idParam(value);
  const user = id === undefined || id.trim() === "" ? Number.NaN : Number(id);
  return Number.isSafeInteger(user) ? user : undefined;
}

/** The page `?settings=` asks for: the first Section for a word no Section has. */
function pageParam(value: unknown): SettingsPage | undefined {
  if (value === undefined || value === allSections) {
    return value;
  }
  return sections.find((each) => each === value) ?? firstSection;
}
