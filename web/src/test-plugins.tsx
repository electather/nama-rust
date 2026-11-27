import type { MessageInitShape } from "@bufbuild/protobuf";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { expect } from "vite-plus/test";

import type { PluginSchema } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { libraries, plugins } from "@/test-plugin-list";
import { admin, fakeServer } from "@/test-server";
import type { Answers } from "@/test-server-api";

/**
 * Opens Settings on its Plugins section, over a page that asks the Server for nothing, with the
 * Plugin `open` in place of the list if one is named, on a Server with `plugins` that answers
 * `answers` too.
 */
export function openPluginSettings(answers: Answers = {}, open = "") {
  const asked = open === "" ? "" : `&plugin=${open}`;
  const server = fakeServer(
    { signedIn: [Permission.MANAGE_SETTINGS], libraries, plugins },
    answers,
  );
  return openApp(`/code?settings=plugins${asked}`, server.transport);
}

/**
 * Opens the Plugin `shown` in Settings' Plugins section, as `openPluginSettings` does, signed in
 * as `signedIn`: one of `plugins` by id, or a record in place of the one with its id.
 */
export function openPlugin(
  shown: MessageInitShape<typeof PluginSchema> | string,
  answers: Answers = {},
  signedIn: Permission[] = [Permission.MANAGE_SETTINGS],
) {
  const id = typeof shown === "string" ? shown : (shown.summary?.id ?? "");
  const others = plugins.filter((each) => each.summary.id !== id);
  const held = typeof shown === "string" ? plugins : [...others, shown];
  const server = fakeServer({ signedIn, libraries, plugins: held }, answers);
  return openApp(`/code?settings=plugins&plugin=${id}`, server.transport);
}

/** Opens the Plugin `shown` as `openPlugin` does, signed in as an Admin, returning the app's router. */
export function openPluginAsAdmin(
  shown: MessageInitShape<typeof PluginSchema> | string,
  answers: Answers = {},
) {
  return openPlugin(shown, answers, admin);
}

/** The Plugins list's row for the Plugin shown as `name`: its name, or its id without one. */
export async function row(name: string) {
  const listed = (await screen.findByRole("link", { name })).closest("li");
  if (listed === null) {
    throw new Error(`${name} has no row`);
  }
  return within(listed);
}

/** The view of the Plugin shown as `name`, as `row` finds it, once it shows. */
export async function plugin(name: string) {
  return within(await screen.findByRole("region", { name }));
}

/** Waits for the Plugin's details, as the view shows before them, saying "Loading…". */
export async function waitForDetails(page: Awaited<ReturnType<typeof plugin>>) {
  await waitFor(() => {
    expect(page.queryByText("Loading…")).toBeNull();
  });
}

/** The input for the setting labelled `label` in the view of the Plugin shown as `name`. */
export async function input(name: string, label: string) {
  // The view shows before its settings, which wait for the Plugin's details.
  return (await plugin(name)).findByLabelText<HTMLInputElement>(label);
}

/** Sends the settings form in the view of the Plugin shown as `name`, as changing any does. */
export async function saveSettings(name: string) {
  const view = await plugin(name);
  // The view shows before its settings form, which waits for the Plugin's details.
  const form = (await view.findByRole("heading", { name: "Settings" })).closest("form");
  if (form === null) {
    throw new Error(`${name} has no settings form`);
  }
  fireEvent.submit(form);
}

/** The input of the chips field labelled `label` on the Plugin `name`'s page, once it shows. */
export async function chipsField(name: string, label: string) {
  const page = await plugin(name);
  return page.findByRole<HTMLInputElement>("combobox", { name: label });
}

/** The labels of what the chips field `field` has picked, in order. */
export function pickedIn(field: HTMLElement) {
  const chips = field
    .closest("[data-slot=combobox-chips]")
    ?.querySelectorAll("[data-slot=combobox-chip]");
  return [...(chips ?? [])].map((chip) => chip.textContent);
}

/** Opens the list of the chips field `field`, as the down arrow does, once it shows. */
async function openList(field: HTMLElement) {
  fireEvent.keyDown(field, { key: "ArrowDown" });
  return within(await screen.findByRole("listbox"));
}

/** The labels of what the chips field `field` offers, in order, from its list. */
export async function offeredIn(field: HTMLElement) {
  const list = await openList(field);
  const labels = list.getAllByRole("option").map((option) => option.textContent);
  fireEvent.keyDown(field, { key: "Escape" });
  return labels;
}

/**
 * Picks `label` in the chips field `field` from its list, or removes it if it's picked, then
 * closes the list, which hides the rest of the page while it's open.
 */
export async function toggleIn(field: HTMLElement, label: string) {
  const list = await openList(field);
  fireEvent.click(list.getByRole("option", { name: label }));
  fireEvent.keyDown(field, { key: "Escape" });
}
