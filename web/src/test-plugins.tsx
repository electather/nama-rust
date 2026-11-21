import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";

import type { LibraryService } from "@/gen/nama/v1/library_pb";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, openApp } from "@/test-app";
import { libraries, plugins, summaryOf } from "@/test-plugin-list";

/**
 * A fake Server with `libraries` that lists `plugins`, gets each by id, and answers other calls
 * with `changes`, and other `LibraryService` calls, such as listing the Libraries, with `library`.
 * A User with `signedIn` is signed in to it: by default, one who manages settings.
 */
function pluginServer(
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

/** Opens the Plugins list on a fake Server with `plugins`, answering other calls with `changes`. */
export function openPlugins(changes: Partial<ServiceImpl<typeof PluginService>> = {}) {
  openApp("/admin/plugins", pluginServer(changes));
}

/**
 * Opens the Plugin `id`'s details page, as `openPlugins` does the list, answering `LibraryService`
 * calls with `library` where it gives them.
 */
export function openPlugin(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>> = {},
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
) {
  openApp(`/admin/plugins/${id}`, pluginServer(changes, library));
}

/** Opens the Plugin `id`'s details page as `openPlugin` does, signed in as an Admin. */
export function openPluginAsAdmin(
  id: string,
  changes: Partial<ServiceImpl<typeof PluginService>> = {},
) {
  openApp(`/admin/plugins/${id}`, pluginServer(changes, {}, [Permission.ADMIN]));
}

/** The Plugins list's row for the Plugin shown as `name`: its name, or its id without one. */
export async function row(name: string) {
  const listed = (await screen.findByRole("link", { name })).closest("li");
  if (listed === null) {
    throw new Error(`${name} has no row`);
  }
  return within(listed);
}

/** The details page of the Plugin shown as `name`, as `row` finds it, once it shows. */
export async function plugin(name: string) {
  await screen.findByRole("heading", { level: 1, name });
  return within(screen.getByRole("main"));
}

/** The input for the setting labelled `label` on the details page of the Plugin shown as `name`. */
export async function input(name: string, label: string) {
  return (await plugin(name)).getByLabelText<HTMLInputElement>(label);
}

/**
 * Sends the settings form on the details page of the Plugin shown as `name`, as changing any of
 * its settings does.
 */
export async function saveSettings(name: string) {
  const page = await plugin(name);
  const form = page.getByRole("heading", { name: "Settings" }).closest("form");
  if (form === null) {
    throw new Error(`${name} has no settings form`);
  }
  fireEvent.submit(form);
}
