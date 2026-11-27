import type { Transport } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";

import type { SettingsPage } from "@/features/settings/lib/sections";
import { openApp } from "@/test-app";

/** Opens the app with Settings open on `section`, over a page that asks the Server for nothing. */
export function openSettings(section: SettingsPage, transport: Transport) {
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

/** Opens Settings' Libraries on their list, or on what `library` opens: an id, or `new`. */
export function openLibraries(transport: Transport, library?: string) {
  const open = library === undefined ? "" : `&library=${library}`;
  return openApp(`/code?settings=libraries${open}`, transport);
}

/** The link named `name` in Settings, once it shows. The top bar under it links each Library too. */
export async function findSettingsLink(name: string) {
  const settings = await screen.findByRole("dialog", { name: "Settings" });
  return within(settings).findByRole("link", { name });
}
