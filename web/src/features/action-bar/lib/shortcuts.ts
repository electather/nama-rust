import type { Page } from "@/features/action-bar/lib/pages";
import { type Group, type Row, matching, section } from "@/features/action-bar/lib/rows";
import { modKey } from "@/shared/lib/mod-key";

function shortcut(label: string, hint: string, chips: readonly string[]): Row {
  return { id: `shortcut:${chips.join("")}`, label, hint, chips };
}

const menu = [
  shortcut("Toggle command menu", "Open or close the command menu from anywhere", [modKey, "K"]),
  shortcut("Open command menu", "Open the command menu when not typing in a field", ["/"]),
  shortcut("Close menu", "Close the menu, or step back one drill level", ["Esc"]),
];

const actions = [shortcut("Keyboard shortcuts", "View every active hotkey", [modKey, "/"])];

/**
 * Every key the action bar answers to (`hooks/use-action-bar-keys.ts`, `hooks/use-action-bar.ts`),
 * for its keyboard shortcuts frame. A key added there is added here.
 */
export function shortcutGroups(query: string, pages: readonly Page[]): Group[] {
  const jumps = pages.map((page) => shortcut(`Jump to ${page.label}`, "", ["G", "→", page.key]));
  return [
    ...section("Menu", matching(query, menu)),
    ...section("Navigate", matching(query, jumps)),
    ...section("Actions", matching(query, actions)),
  ];
}
