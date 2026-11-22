import { fireEvent, screen } from "@testing-library/react";

import type { Section } from "@/features/settings/lib/sections";
import { type fakeServer, openApp } from "@/test-app";

/** Opens the app with Settings open on `section`, over a page that asks the Server for nothing. */
export function openSettings(section: Section, transport: ReturnType<typeof fakeServer>) {
  return openApp(`/code?settings=${section}`, transport);
}

/** The dropdown labelled `label`, whose text is the option picked. */
export function dropdown(label: string) {
  return screen.getByRole("combobox", { name: label });
}

/** Picks the option named `name`, or whose name matches it, in the dropdown labelled `label`. */
export function pick(label: string, name: RegExp | string) {
  fireEvent.click(dropdown(label));
  const option = screen.getByRole("option", { name });
  // The list takes a click only from a press that began on the option.
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

/** The names of the options the dropdown labelled `label` offers, in order, from its list. */
export async function offered(label: string) {
  fireEvent.click(dropdown(label));
  const options = await screen.findAllByRole("option");
  const names = options.map((option) => option.textContent);
  fireEvent.keyDown(options[0] ?? document.body, { key: "Escape" });
  return names;
}
