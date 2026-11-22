import type { UseNavigateResult } from "@tanstack/react-router";

import type { IconName } from "@/features/action-bar/lib/rows";

/** The pages "Jump to" lists, each with the key that, after G, goes there from anywhere. */
const pages = [
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
    // Settings is a modal over the page it's opened on (`goTo`), not a page of its own.
    to: ".",
    label: "Settings",
    hint: "Your password, Devices and playback",
    key: "S",
    icon: "settings",
  },
  {
    to: "/admin",
    label: "Admin",
    hint: "Libraries and Needs a Match",
    key: "A",
    icon: "admin",
  },
] as const satisfies readonly {
  to: string;
  label: string;
  hint: string;
  key: string;
  icon: IconName;
}[];

/** A page the action bar jumps to. */
export type Page = (typeof pages)[number];

/** The pages a User may jump to: Admin only for one who may open some Admin page. */
export function pagesFor(hasAdminPages: boolean): Page[] {
  return pages.filter((page) => hasAdminPages || page.to !== "/admin");
}

/** Goes to `page`. Settings opens in a history entry of its own, so Back closes it. */
export function goTo(navigate: UseNavigateResult<string>, page: Page) {
  void (page.to === "."
    ? navigate({ to: ".", search: (rest) => ({ ...rest, settings: "playback" }) })
    : navigate({ to: page.to }));
}
