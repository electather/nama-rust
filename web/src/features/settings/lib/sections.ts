import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import {
  type Section,
  type SettingsPage,
  allSections,
  firstSection,
} from "@/shared/lib/settings-address";

/** Each section's name, in the sidebar and over its pane. */
export const sectionTitles: Record<Section, string> = {
  signin: "Sign-in",
  devices: "Devices",
  playback: "Playback",
  home: "Home",
  requests: "Requests",
  connections: "Connections",
  libraries: "Libraries",
  "needs-a-match": "Needs a Match",
  "now-playing": "Now Playing",
  jobs: "Jobs",
  server: "Server",
  users: "Users",
  backups: "Backups",
  "default-home": "Default Home",
  "server-playback": "Server Playback",
  plugins: "Plugins",
  account: "Account",
  "account-signin": "Sign-in",
  "account-sessions": "Sessions",
  "account-servers": "Servers",
  "account-history": "Viewing History",
};

/** What else search finds a section by, past its name, for the sections that have such words. */
export const sectionWords: Partial<Record<Section, string>> = {
  libraries:
    "libraries library folders scan movies shows title sources recognisers segment sources backdrop",
  "needs-a-match": "needs a match unmatched identify fix wrong title",
  "now-playing": "now playing watching streams sessions stop transcode",
  jobs: "jobs schedule run tasks upkeep",
  requests: "requests quality withdraw approve decline pending followed shows",
  account: "email delete",
  "account-signin": "password passkeys",
  "account-sessions": "sign out browsers",
  "account-servers": "linked unlink link code invites accept",
  "account-history": "viewing events plays export pooled counts",
};

/** The sidebar's sections under a heading. */
export interface SectionGroup {
  label: string;
  items: Section[];
}

/** The Permission a section needs, for the ones not every User sees. */
const needs: Partial<Record<Section, Permission>> = {
  libraries: Permission.MANAGE_LIBRARIES,
  "needs-a-match": Permission.MANAGE_LIBRARIES,
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
  { label: "Media", items: ["libraries", "needs-a-match"] },
  { label: "Activity", items: ["now-playing", "jobs"] },
  {
    label: "Admin",
    items: ["server", "users", "backups", "default-home", "server-playback", "plugins"],
  },
];

// The Account's own Sections, only at the Hub (ADR 0105), where no Server's are.
const hubGroups: SectionGroup[] = [
  {
    label: "Your Account",
    items: ["account", "account-signin", "account-sessions", "account-servers", "account-history"],
  },
];

/** The sections `user` may open, grouped as the sidebar lists them; at the Hub, the Account's. */
export function groupsFor(user: User | undefined, atHub: boolean): SectionGroup[] {
  if (atHub) {
    return hubGroups;
  }
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

/**
 * `asked` if `from` lists it, else the first section, or at the Hub, which lists no Playback, the
 * first `from` lists: a section `user` may not open isn't shown.
 */
export function openSection(from: SectionGroup[], asked: SettingsPage): Section {
  const listed = from.flatMap((group) => group.items);
  const wanted = asked === allSections ? firstSection : asked;
  return [wanted, firstSection].find((section) => listed.includes(section)) ?? listed[0] ?? wanted;
}
