// The rule this module keeps is in `web/src/AGENTS.md` ("Only Settings' modal tree imports the Section
// definitions").
import {
  type SectionDefinition,
  type SectionGroupDefinition,
  everyUser,
} from "@/features/settings/lib/section-definition";
import { activity } from "@/features/settings/lib/section-groups/activity";
import { admin } from "@/features/settings/lib/section-groups/admin";
import { media } from "@/features/settings/lib/section-groups/media";
import { preferences } from "@/features/settings/lib/section-groups/preferences";
import { you } from "@/features/settings/lib/section-groups/you";
import { yourAccount } from "@/features/settings/lib/section-groups/your-account";
import type {
  ServerDefinedSetting,
  ServerDefinedSource,
} from "@/features/settings/lib/server-defined";
import type { Findable } from "@/features/settings/lib/setting-search";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import type { SettingText } from "@/shared/lib/setting-text";
import {
  type Section,
  type SettingsPage,
  allSections,
  firstSection,
  sections,
} from "@/shared/lib/settings-address";

/** Each Section's definition, by its word: the compiler holds it to every word and no other. */
export const definitions: Record<Section, SectionDefinition> = {
  ...you.sections,
  ...preferences.sections,
  ...media.sections,
  ...activity.sections,
  ...admin.sections,
  ...yourAccount.sections,
};

/** The sidebar's sections under a heading. */
export interface SectionGroup {
  label: string;
  items: Section[];
}

function sidebarGroup(group: SectionGroupDefinition): SectionGroup {
  // `Object.keys` gives strings: matching each to its word keeps the definitions' order and type.
  const items = Object.keys(group.sections).flatMap((key) =>
    sections.filter((word) => word === key),
  );
  return { label: group.label, items };
}

const serverGroups = [you, preferences, media, activity, admin].map((group) => sidebarGroup(group));

const hubGroups = [sidebarGroup(yourAccount)];

/** The sections `user` may open, grouped as the sidebar lists them; at the Hub, the Account's. */
export function groupsFor(user: User | undefined, atHub: boolean): SectionGroup[] {
  if (atHub) {
    return hubGroups;
  }
  return serverGroups
    .map(({ label, items }) => ({ label, items: items.filter((section) => opens(user, section)) }))
    .filter((group) => group.items.length > 0);
}

function opens(user: User | undefined, section: Section) {
  const { permission } = definitions[section];
  return permission === everyUser || may(user, permission);
}

/**
 * `asked` if `from` lists it, else the first section, or at the Hub, which lists no Playback, the
 * first `from` lists: a section `user` may not open isn't shown.
 */
export function openSection(from: SectionGroup[], asked: SettingsPage): Section {
  const shown = from.flatMap((group) => group.items);
  const wanted = asked === allSections ? firstSection : asked;
  return [wanted, firstSection].find((section) => shown.includes(section)) ?? shown[0] ?? wanted;
}

/** Where search finds the settings the Server defines in the Sections of `groups`. */
export function serverDefinedIn(groups: SectionGroup[]): ServerDefinedSource[] {
  return groups
    .flatMap((group) => group.items)
    .flatMap((section) => definitions[section].serverDefined ?? []);
}

/** What the built-in setting `key` of `section` says, if it has one by that key. */
export function builtInText(section: Section, key: string): SettingText | undefined {
  const { labels } = definitions[section];
  return Object.hasOwn(labels, key) ? labels[key] : undefined;
}

/**
 * Each of the Sections in `groups` and the settings in them `user` sees, in the sidebar's order and
 * each pane's: a Section, its built-in settings, then those of `serverDefined` in it.
 */
export function findables(
  groups: SectionGroup[],
  user: User | undefined,
  serverDefined: ServerDefinedSetting[],
): Findable[] {
  return groups
    .flatMap((group) => group.items)
    .flatMap((section) => inSection(section, user, serverDefined));
}

/** `section` and the settings in it `user` sees, built-in ones first, as its pane lists them. */
function inSection(
  section: Section,
  user: User | undefined,
  serverDefined: ServerDefinedSetting[],
): Findable[] {
  const { title, words = "", labels, serverDefined: source } = definitions[section];
  const builtInShown = Object.entries(labels)
    .filter(([, text]) => text.needs === undefined || may(user, text.needs))
    .map(([key, text]): Findable => ({ section, key, text }));
  return [
    { section, text: { label: title, words } },
    ...builtInShown,
    ...serverDefined
      .filter((setting) => setting.source === source)
      .map(({ key, text }) => ({ section, key, text })),
  ];
}
