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
} & ({ to: "/" | "/requests" } | { settings: Section });

/** The pages "Jump to" lists to everyone. */
const pages: readonly Page[] = [
  {
    to: "/",
    label: "Home",
    hint: "Continue Watching and your Libraries",
    key: "H",
    icon: "home",
  },
  {
    to: "/requests",
    label: "Requests",
    hint: "Ask for a Movie or Show, and follow it",
    key: "R",
    icon: "requests",
  },
  {
    settings: "playback",
    label: "Settings",
    hint: "Your password, Devices and playback",
    key: "S",
    icon: "settings",
  },
];

/** The pages a User may jump to: Admin, at `adminSection`, only for one who may open some. */
export function pagesFor(adminSection: Section | undefined): Page[] {
  if (adminSection === undefined) {
    return [...pages];
  }
  const admin: Page = {
    settings: adminSection,
    label: "Admin",
    hint: "What you manage on this Server",
    key: "A",
    icon: "admin",
  };
  return [...pages, admin];
}

/** Goes to `page`. Settings opens in a history entry of its own, so Back closes it. */
export function goTo(navigate: UseNavigateResult<string>, page: Page) {
  if ("to" in page) {
    void navigate({ to: page.to });
    return;
  }
  const { settings } = page;
  void navigate({ to: ".", search: (rest) => ({ ...rest, ...sectionSearch(settings) }) });
}
