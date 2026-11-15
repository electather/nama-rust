import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginKind, PluginPermission, PluginService } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { openPlugin, openPlugins, plugin, row } from "./test-plugins";
import { refused } from "./test-refusal";

afterEach(cleanup);

test.each([
  ["ending in a full stop shows one", "the key was turned down."],
  ["without a full stop gets one", "the key was turned down"],
])("a refused-to-start reason %s", async (_, reason) => {
  openPlugins({
    enablePlugin: () => {
      throw refused(Code.FailedPrecondition, "test-plugin refused to start", {
        reason: RefusalReason.PLUGIN_REFUSED_TO_START,
        field: "id",
        params: { pluginId: "test-plugin", reason },
      });
    },
  });
  const testPlugin = await row("test-plugin");
  fireEvent.click(testPlugin.getByRole("button", { name: "Enable test-plugin" }));

  const alert = await testPlugin.findByRole("alert");
  expect(alert.textContent).toBe("It refused to start: the key was turned down.");
});

/** An Installed cleanup Plugin requesting `requestedPermissions`, listed alone, disabled by default. */
function cleanupPlugin(
  requestedPermissions: PluginPermission[],
  granted: { enabled?: boolean; grantedPermissions?: PluginPermission[] } = {},
) {
  const summary = {
    id: "cleanup",
    version: "0.1.0",
    kind: PluginKind.INSTALLED,
    enabled: false,
    requestedPermissions,
    ...granted,
  };
  return {
    listPlugins: () => ({ summaries: [summary] }),
    getPlugin: () => ({ plugin: { summary } }),
  };
}

/** Opens the list with the cleanup Plugin and returns the `GrantedPermissions` each Enable sends. */
function openWithCleanup(requested: PluginPermission[]) {
  const sent: PluginPermission[][] = [];
  const changes: Partial<ServiceImpl<typeof PluginService>> = {
    ...cleanupPlugin(requested),
    enablePlugin: ({ grantedPermissions }) => {
      sent.push(grantedPermissions);
      return {};
    },
  };
  openPlugins(changes);
  return sent;
}

test("enabling a Plugin that requests Plugin Permissions lists them, none checked", async () => {
  openWithCleanup([PluginPermission.READ_WATCH_STATES, PluginPermission.DELETE_MEDIA]);
  fireEvent.click((await row("cleanup")).getByRole("button", { name: "Enable cleanup" }));

  const dialog = within(await screen.findByRole("alertdialog", { name: "Enable cleanup?" }));
  const boxes = dialog.getAllByRole<HTMLInputElement>("checkbox");
  expect(boxes.map((box) => box.labels?.[0]?.textContent)).toStrictEqual([
    "Read Watch State",
    "Delete media",
  ]);
  expect(boxes.map((box) => box.checked)).toStrictEqual([false, false]);
});

test("enabling sends only the Plugin Permissions checked", async () => {
  const sent = openWithCleanup([
    PluginPermission.STORE,
    PluginPermission.READ_LIBRARIES,
    PluginPermission.DELETE_MEDIA,
  ]);
  fireEvent.click((await row("cleanup")).getByRole("button", { name: "Enable cleanup" }));

  const dialog = within(await screen.findByRole("alertdialog", { name: "Enable cleanup?" }));
  fireEvent.click(dialog.getByLabelText("Read Libraries"));
  fireEvent.click(dialog.getByLabelText("Keep its own data"));
  fireEvent.click(dialog.getByRole("button", { name: "Enable" }));

  await waitFor(() => {
    expect(sent).toStrictEqual([[PluginPermission.STORE, PluginPermission.READ_LIBRARIES]]);
  });
});

test("a Plugin requesting no Plugin Permissions enables without a dialog", async () => {
  const sent = openWithCleanup([]);
  fireEvent.click((await row("cleanup")).getByRole("button", { name: "Enable cleanup" }));

  await waitFor(() => {
    expect(sent).toStrictEqual([[]]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("an enabled Plugin's details page lists the Plugin Permissions granted it", async () => {
  openPlugin(
    "cleanup",
    cleanupPlugin([PluginPermission.READ_WATCH_STATES, PluginPermission.DELETE_MEDIA], {
      enabled: true,
      grantedPermissions: [PluginPermission.READ_WATCH_STATES],
    }),
  );

  expect((await plugin("cleanup")).getByText("Granted: Read Watch State")).toBeDefined();
});
