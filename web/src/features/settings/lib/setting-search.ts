import { answers, typedWords } from "@/features/settings/lib/search-words";
import type { SettingText } from "@/shared/lib/setting-text";
import type { Section } from "@/shared/lib/settings-address";

/**
 * A setting, or a Section, that Settings' search finds: its Section, the key `?setting=` finds its
 * row by, and what its row says. A Section's own has no key, and is found by its name alone. A
 * Plugin's own settings and a Plugin Connection aren't searched.
 */
export interface Findable {
  section: Section;
  key?: string;
  text: SettingText;
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
