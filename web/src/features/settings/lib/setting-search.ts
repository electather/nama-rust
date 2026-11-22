import { backupLabels } from "@/features/admin-backups/lib/backup-settings";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import { notificationLabels } from "@/features/settings/lib/notification-labels";
import { deviceLabels, signinLabels, userLabels } from "@/features/settings/lib/pane-labels";
import { playbackLabels } from "@/features/settings/lib/playback-values";
import { answers, typedWords } from "@/features/settings/lib/search-words";
import {
  type Section,
  type SectionGroup,
  sectionTitles,
  sectionWords,
} from "@/features/settings/lib/sections";
import { serverLabels } from "@/features/settings/lib/server-values";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { homeLabels } from "@/shared/lib/home-layout";
import { may } from "@/shared/lib/permissions";
import type { SettingText } from "@/shared/lib/setting-text";

/**
 * A setting, or a Section, that Settings' search finds: its Section, the key `?setting=` finds its
 * row by, and what its row says. A Section's own has no key, and is found by its name alone.
 */
export interface Findable {
  section: Section;
  key?: string;
  text: SettingText;
}

// Each Section's built-in settings, by the label maps their rows read. A Plugin's own settings and
// a Plugin Connection aren't searched; those the Server defines join from `server-defined.ts`.
const builtIn: Record<Section, Record<string, SettingText>> = {
  signin: signinLabels,
  devices: deviceLabels,
  playback: playbackLabels,
  home: homeLabels,
  requests: {},
  connections: notificationLabels,
  "needs-a-match": {},
  "now-playing": {},
  jobs: {},
  server: serverLabels,
  users: userLabels,
  backups: backupLabels,
  "default-home": homeLabels,
  "server-playback": serverPlaybackLabels,
  plugins: {},
};

/** What the built-in setting `key` of `section` says, if it has one by that key. */
export function builtInText(section: Section, key: string): SettingText | undefined {
  return Object.hasOwn(builtIn[section], key) ? builtIn[section][key] : undefined;
}

/**
 * Each of the Sections in `groups` and the settings in them `user` sees, in the sidebar's order and
 * each pane's: a Section, its built-in settings, then those of `serverDefined` in it.
 */
export function findables(
  groups: SectionGroup[],
  user: User | undefined,
  serverDefined: Findable[],
): Findable[] {
  return groups
    .flatMap((group) => group.items)
    .flatMap((section) => inSection(section, user, serverDefined));
}

/** `section` and the settings in it `user` sees, built-in ones first, as its pane lists them. */
function inSection(section: Section, user: User | undefined, serverDefined: Findable[]) {
  const builtInShown = Object.entries(builtIn[section])
    .filter(([, text]) => text.needs === undefined || may(user, text.needs))
    .map(([key, text]): Findable => ({ section, key, text }));
  return [
    { section, text: { label: sectionTitles[section], words: sectionWords[section] ?? "" } },
    ...builtInShown,
    ...serverDefined.filter((findable) => findable.section === section),
  ];
}

/** What tells `findable` from the others, its Section's own included. */
export function findableId({ section, key }: Findable) {
  return `${section}/${key ?? ""}`;
}

/** Those of `from` that answer to every word `typed`: none when it holds no word. */
export function matching(from: Findable[], typed: string): Findable[] {
  const asked = typedWords(typed);
  return asked.length === 0 ? [] : from.filter((findable) => answers(findable.text, asked));
}
