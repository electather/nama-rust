import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/** Each section of Settings, by the word `?settings=` opens it with. */
const sections = [
  "signin",
  "devices",
  "playback",
  "home",
  "requests",
  "connections",
  "now-playing",
  "jobs",
  "server",
  "users",
  "backups",
  "default-home",
  "server-playback",
  "plugins",
] as const;

/** A section of Settings. */
export type Section = (typeof sections)[number];

/** The section Settings opens on when asked for one it doesn't have. */
const firstSection: Section = "playback";

/** The section `?settings=` asks for: none without it, and the first for a word no section has. */
export function sectionParam(value: unknown): Section | undefined {
  if (value === undefined) {
    return undefined;
  }
  return sections.find((each) => each === value) ?? firstSection;
}

/**
 * The Plugin `?plugin=` asks the Plugins section to open, the Job `?job=` asks the Jobs section to
 * open, or the setting `?setting=` asks for: none without it.
 */
export function idParam(value: unknown) {
  // The router reads an id made of digits as a number.
  return typeof value === "string" || typeof value === "number" ? String(value) : undefined;
}

/** The address of `section` in Settings, at its `setting` if one is asked for. A Plugin, User or Job open in another Section closes. */
export function sectionSearch(section: Section, setting?: string) {
  return { settings: section, plugin: undefined, user: undefined, job: undefined, setting };
}

/** The User `?user=` asks the Users section to open: none without it. */
export function userParam(value: unknown) {
  // A number, as the router writes a string of digits quoted: `user=%222%22`, not `user=2`.
  const id = typeof value === "string" || typeof value === "number" ? Number(value) : Number.NaN;
  return Number.isSafeInteger(id) ? id : undefined;
}

/** Each section's name, in the sidebar and over its pane. */
export const sectionTitles: Record<Section, string> = {
  signin: "Sign-in",
  devices: "Devices",
  playback: "Playback",
  home: "Home",
  requests: "Requests",
  connections: "Connections",
  "now-playing": "Now Playing",
  jobs: "Jobs",
  server: "Server",
  users: "Users",
  backups: "Backups",
  "default-home": "Default Home",
  "server-playback": "Server Playback",
  plugins: "Plugins",
};

/** What a pane that didn't load says it couldn't load. */
export const loadNouns: Record<Section, string> = {
  signin: "your sign-in details",
  devices: "your Devices",
  playback: "your Playback settings",
  home: "your Home layout",
  requests: "your Requests settings",
  connections: "your Connections",
  "now-playing": "what’s playing",
  jobs: "the Jobs",
  server: "the Server settings",
  users: "the Users",
  backups: "the Backups",
  "default-home": "the default Home layout",
  "server-playback": "the Server’s Playback settings",
  plugins: "the Plugins",
};

/** What else search finds a section by, past its name, for the sections that have such words. */
export const sectionWords: Partial<Record<Section, string>> = {
  "now-playing": "now playing watching streams sessions stop transcode",
  jobs: "jobs schedule run tasks upkeep",
};

/** The sidebar's sections under a heading. */
export interface SectionGroup {
  label: string;
  items: Section[];
}

/** The Permission a section needs, for the ones not every User sees. */
const needs: Partial<Record<Section, Permission>> = {
  requests: Permission.CHOOSE_QUALITY,
  "now-playing": Permission.ADMIN,
  jobs: Permission.ADMIN,
  server: Permission.MANAGE_SETTINGS,
  users: Permission.MANAGE_USERS,
  backups: Permission.ADMIN,
  "default-home": Permission.MANAGE_SETTINGS,
  "server-playback": Permission.ADMIN,
  plugins: Permission.MANAGE_SETTINGS,
};

const groups: SectionGroup[] = [
  { label: "You", items: ["signin", "devices"] },
  { label: "Preferences", items: ["playback", "home", "requests", "connections"] },
  { label: "Activity", items: ["now-playing", "jobs"] },
  {
    label: "Admin",
    items: ["server", "users", "backups", "default-home", "server-playback", "plugins"],
  },
];

/** The sections `user` may open, grouped as the sidebar lists them. */
export function groupsFor(user: User | undefined): SectionGroup[] {
  return shownGroups((section) => {
    const permission = needs[section];
    return permission === undefined || may(user, permission);
  });
}

/** The groups with only the sections `keep` keeps, without the groups left empty. */
function shownGroups(keep: (section: Section) => boolean): SectionGroup[] {
  return groups
    .map(({ label, items }) => ({ label, items: items.filter((section) => keep(section)) }))
    .filter((group) => group.items.length > 0);
}

/** `asked` if `from` lists it, else the first section: a section `user` may not open isn't shown. */
export function openSection(from: SectionGroup[], asked: Section): Section {
  return from.some((group) => group.items.includes(asked)) ? asked : firstSection;
}
