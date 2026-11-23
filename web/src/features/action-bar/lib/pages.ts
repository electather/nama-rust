import type { UseNavigateResult } from "@tanstack/react-router";

import type { IconName } from "@/features/action-bar/lib/rows";
import { type Section, sectionSearch } from "@/features/settings/lib/sections";

/** A page the action bar jumps to, with the key that, after G, goes there from anywhere. */
export type Page = {
  label: string;
  hint: string;
  key: string;
  icon: IconName;
  // Settings is a modal over the page it's opened on (`goTo`), not a page of its own.
} & ({ to: "/" } | { settings: Section });

/** The pages "Jump to" lists. */
export const pages: readonly Page[] = [
  {
    to: "/",
    label: "Home",
    hint: "Continue Watching and your Libraries",
    key: "H",
    icon: "home",
  },
  {
    settings: "playback",
    label: "Settings",
    hint: "Your password, Devices and playback",
    key: "S",
    icon: "settings",
  },
];

/** Goes to `page`. Settings opens in a history entry of its own, so Back closes it. */
export function goTo(navigate: UseNavigateResult<string>, page: Page) {
  if ("to" in page) {
    void navigate({ to: page.to });
    return;
  }
  const { settings } = page;
  void navigate({ to: ".", search: (rest) => ({ ...rest, ...sectionSearch(settings) }) });
}
