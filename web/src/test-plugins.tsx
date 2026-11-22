import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type HandlerContext, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { expect } from "vite-plus/test";

import type { LibraryService } from "@/gen/nama/v1/library_pb";
import { GetPluginRequestSchema, PluginService } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, openApp } from "@/test-app";
import { libraries, plugins, summaryOf } from "@/test-plugin-list";

/**
 * A fake Server with `libraries` that lists `plugins`, gets each by id, and answers other calls
 * with `changes`, and other `LibraryService` calls, such as listing the Libraries, with `library`.
 * A User with `signedIn` is signed in to it: by default, one who manages settings.
 */
export function pluginServer(
  changes: Partial<ServiceImpl<typeof PluginService>>,
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
  signedIn: Permission[] = [Permission.MANAGE_SETTINGS],
) {
  const state = { signedIn, libraries, libraryService: library };
  return fakeServer(state, {}, {}, (router) => {
    router.service(PluginService, {
      listPlugins: () => ({ summaries: plugins.map((listed) => summaryOf(listed)) }),
      getPlugin: ({ id }) => {
        const found = plugins.find((listed) => listed.id === id);
        if (!found) {
          throw new ConnectError(`there is no Plugin ${id}`, Code.NotFound);
        }
        // Sending it drops what `Plugin` has no field for: those are the summary's.
        return { plugin: { ...found, summary: summaryOf(found) } };
      },
      ...changes,
    });
  });
}

/**
 * Opens Settings on its Plugins section, over a page that asks the Server for nothing, with the
 * Plugin `open` in place of the list if one is named, answering other calls with `changes`.
 */
export function openPluginSettings(
  changes: Partial<ServiceImpl<typeof PluginService>> = {},
  open = "",
) {
  const asked = open === "" ? "" : `&plugin=${open}`;
  return openApp(`/code?settings=plugins${asked}`, pluginServer(changes));
}

/**
 * Opens the Plugin `id` in Settings' Plugins section, as `openPluginSettings` does, answering
 * `LibraryService` calls with `library` where it gives them.
 */
export function openPlugin(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>> = {},
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
) {
  openApp(`/code?settings=plugins&plugin=${id}`, pluginServer(listingToo(id, changes), library));
}

/** Opens the Plugin `id` as `openPlugin` does, signed in as an Admin. */
export function openPluginAsAdmin(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>> = {},
) {
  const server = pluginServer(listingToo(id, changes), {}, [Permission.ADMIN]);
  openApp(`/code?settings=plugins&plugin=${id}`, server);
}

/**
 * `changes`, with `listPlugins` listing `id` as a test's `getPlugin` describes it, as the Server's
 * list and details agree: Settings opens only a Plugin it lists, and draws its header from the list.
 */
function listingToo(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>>,
): Partial<ServiceImpl<typeof PluginService>> {
  const all = plugins.map((each) => summaryOf(each));
  const listed = all.filter((each) => each.id !== id);
  const fallback = all.find((each) => each.id === id) ?? { id };
  return {
    listPlugins: async (_request, context) => {
      // A `getPlugin` that refuses lists it as `plugins` does.
      const own = await ownSummary(id, changes, context).catch(() => null);
      return { summaries: [...listed, own ?? fallback] };
    },
    ...changes,
  };
}

/** The summary a test's own `getPlugin`, if it has one, gives the Plugin `id`. */
async function ownSummary(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>>,
  context: HandlerContext,
) {
  const answer = await changes.getPlugin?.(create(GetPluginRequestSchema, { id }), context);
  return answer?.plugin?.summary;
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
