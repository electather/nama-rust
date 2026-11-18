import type { Page } from "@/features/action-bar/lib/menu";

const everyone: Page[] = [
  {
    label: "Home",
    hint: "Continue Watching and your Libraries",
    letter: "H",
    icon: "home",
    to: "/",
  },
  {
    label: "Requests",
    hint: "What’s been asked for",
    letter: "R",
    icon: "requests",
    to: "/requests",
  },
  {
    label: "Settings",
    hint: "Your Devices, password and playback",
    letter: "S",
    icon: "settings",
    to: "/settings",
  },
];

const admin: Page = {
  label: "Admin",
  hint: "Users, Libraries and Plugins",
  letter: "A",
  icon: "admin",
  to: "/admin",
};

/** The pages the action bar jumps to: Admin only for a User who may open some Admin page. */
export function pagesFor(hasAdminPages: boolean) {
  return hasAdminPages ? [...everyone, admin] : everyone;
}
