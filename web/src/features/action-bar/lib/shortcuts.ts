import type { Group, Page, Row } from "@/features/action-bar/lib/menu";
import { shortcutsRow } from "@/features/action-bar/lib/rows";
import { modKey } from "@/shared/lib/mod-key";

function shortcut(label: string, hint: string, chips: string[]): Row {
  return {
    key: `shortcut:${label}`,
    label,
    hint: hint === "" ? undefined : hint,
    art: undefined,
    chips,
    pick: { kind: "nothing" },
  };
}

/** Every shortcut the action bar answers, under what it's for. */
export function shortcutGroups(pages: Page[]): Group[] {
  return [
    {
      heading: "Menu",
      rows: [
        shortcut("Toggle command menu", "Open or close the command menu from anywhere", [
          modKey,
          "K",
        ]),
        shortcut("Open command menu", "Open the command menu when not typing in a field", ["/"]),
        shortcut("Close menu", "Close the menu, or step back one drill level", ["Esc"]),
      ],
    },
    {
      heading: "Navigate",
      rows: pages.map((page) => shortcut(`Jump to ${page.label}`, "", ["G", "→", page.letter])),
    },
    {
      heading: "Actions",
      rows: [shortcut(shortcutsRow.label, shortcutsRow.hint ?? "", shortcutsRow.hotkey ?? [])],
    },
  ];
}
