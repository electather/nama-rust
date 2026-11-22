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
  "server",
  "default-home",
  "plugins",
] as const;

/** A section of Settings. */
export type Section = (typeof sections)[number];

/** The section Settings opens on when asked for one it doesn't have. */
export const firstSection: Section = "playback";

/** The section `?settings=` asks for: none without it, and the first for a word no section has. */
export function sectionParam(value: unknown): Section | undefined {
  if (value === undefined) {
    return undefined;
  }
  return sections.find((each) => each === value) ?? firstSection;
}

/** The Plugin `?plugin=` asks the Plugins section to open: none without it. */
export function pluginParam(value: unknown) {
  // The router reads an id made of digits as a number.
  return typeof value === "string" || typeof value === "number" ? String(value) : undefined;
}

/** Each section's name, in the sidebar and over its pane. */
export const sectionTitles: Record<Section, string> = {
  signin: "Sign-in",
  devices: "Devices",
  playback: "Playback",
  home: "Home",
  requests: "Requests",
  connections: "Connections",
  server: "Server",
  "default-home": "Default Home",
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
  server: "the Server settings",
  "default-home": "the default Home layout",
  plugins: "the Plugins",
};

// Words each section answers to in the sidebar's search, beyond its name.
const searchWords: Record<Section, string> = {
  signin: "username password",
  devices: "device revoke sign out tv phone",
  playback: "audio language subtitles sdh forced size background autoplay next episode",
  home: "rows featured layout order continue watching",
  requests: "quality movies shows request",
  connections: "telegram plugin messages notifications personal",
  server: "server url address images resize space storage store",
  "default-home": "default home rows layout order everyone",
  plugins: "plugin telegram install",
};

/** The sidebar's sections under a heading. */
export interface SectionGroup {
  label: string;
  items: Section[];
}

/** The Permission a section needs, for the ones not every User sees. */
const needs: Partial<Record<Section, Permission>> = {
  requests: Permission.CHOOSE_QUALITY,
  server: Permission.MANAGE_SETTINGS,
  "default-home": Permission.MANAGE_SETTINGS,
  plugins: Permission.MANAGE_SETTINGS,
};

const groups: SectionGroup[] = [
  { label: "You", items: ["signin", "devices"] },
  { label: "Preferences", items: ["playback", "home", "requests", "connections"] },
  { label: "Admin", items: ["server", "default-home", "plugins"] },
];

/** The sections `user` may open, grouped as the sidebar lists them. */
export function groupsFor(user: User | undefined): SectionGroup[] {
  return shownGroups((section) => {
    const permission = needs[section];
    return permission === undefined || may(user, permission);
  });
}

/** `from` narrowed to the sections that answer to the words `typed` in the sidebar's search. */
export function matching(from: SectionGroup[], typed: string): SectionGroup[] {
  const words = typed.trim().toLowerCase();
  const shown = new Set(from.flatMap((group) => group.items));
  return shownGroups(
    (section) =>
      shown.has(section) &&
      `${sectionTitles[section]} ${searchWords[section]}`.toLowerCase().includes(words),
  );
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
