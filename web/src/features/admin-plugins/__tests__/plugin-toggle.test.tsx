import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginKind, PluginPermission, type PluginService } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { openPlugin, plugin } from "@/test-plugins";
import { refused } from "@/test-refusal";

afterEach(cleanup);

test.each([
  ["ending in a full stop shows one", "the key was turned down."],
  ["without a full stop gets one", "the key was turned down"],
])("a refused-to-start reason %s", async (_, reason) => {
  openPlugin("test-plugin", {
    enablePlugin: () => {
      throw refused(Code.FailedPrecondition, "test-plugin refused to start", {
        reason: RefusalReason.PLUGIN_REFUSED_TO_START,
        field: "id",
        params: { pluginId: "test-plugin", reason },
      });
    },
  });
  const testPlugin = await plugin("test-plugin");
  fireEvent.click(await testPlugin.findByRole("button", { name: "Enable test-plugin" }));

  const alert = await testPlugin.findByText("It refused to start: the key was turned down.");
  expect(alert.getAttribute("role")).toBe("alert");
});

/**
 * The cleanup Plugin requesting `requestedPermissions`, listed alone, Installed and disabled unless
 * `granted` says otherwise.
 */
function cleanupPlugin(
  requestedPermissions: PluginPermission[],
  granted: { enabled?: boolean; grantedPermissions?: PluginPermission[]; kind?: PluginKind } = {},
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

/** Opens the cleanup Plugin and returns the `GrantedPermissions` each Enable sends. */
function openWithCleanup(requested: PluginPermission[], kind: PluginKind = PluginKind.INSTALLED) {
  const sent: PluginPermission[][] = [];
  const changes: Partial<ServiceImpl<typeof PluginService>> = {
    ...cleanupPlugin(requested, { kind }),
    enablePlugin: ({ grantedPermissions }) => {
      sent.push(grantedPermissions);
      return {};
    },
  };
  openPlugin("cleanup", changes);
  return sent;
}

test("enabling a Plugin that requests Plugin Permissions lists them, none checked", async () => {
  openWithCleanup([
    PluginPermission.READ_WATCH_STATES,
    PluginPermission.READ_USERS,
    PluginPermission.DELETE_MEDIA,
  ]);
  const found1 = await plugin("cleanup");
  fireEvent.click(await found1.findByRole("button", { name: "Enable cleanup" }));

  const dialog = within(await screen.findByRole("alertdialog", { name: "Enable cleanup?" }));
  const boxes = dialog.getAllByRole<HTMLInputElement>("checkbox");
  expect(boxes.map((box) => box.labels?.[0]?.textContent)).toStrictEqual([
    "Read Watch State",
    "Read Users",
    "Delete media",
  ]);
  expect(boxes.map((box) => box.checked)).toStrictEqual([false, false, false]);
});

test("enabling sends only the Plugin Permissions checked", async () => {
  const sent = openWithCleanup([
    PluginPermission.STORE,
    PluginPermission.READ_LIBRARIES,
    PluginPermission.DELETE_MEDIA,
  ]);
  const found2 = await plugin("cleanup");
  fireEvent.click(await found2.findByRole("button", { name: "Enable cleanup" }));

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
  const found3 = await plugin("cleanup");
  fireEvent.click(await found3.findByRole("button", { name: "Enable cleanup" }));

  await waitFor(() => {
    expect(sent).toStrictEqual([[]]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("an enabled Plugin's view lists the Plugin Permissions granted it", async () => {
  openPlugin(
    "cleanup",
    cleanupPlugin([PluginPermission.READ_WATCH_STATES, PluginPermission.DELETE_MEDIA], {
      enabled: true,
      grantedPermissions: [PluginPermission.READ_WATCH_STATES],
    }),
  );

  const found4 = await plugin("cleanup");
  await expect(found4.findByText("Granted: Read Watch State")).resolves.toBeDefined();
});

test("a Built-in Plugin requesting Plugin Permissions enables without a dialog", async () => {
  const sent = openWithCleanup([PluginPermission.STORE], PluginKind.BUILT_IN);
  const found5 = await plugin("cleanup");
  fireEvent.click(await found5.findByRole("button", { name: "Enable cleanup" }));

  await waitFor(() => {
    expect(sent).toStrictEqual([[]]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test.each([true, false])(
  "a Built-in Plugin's view lists the Plugin Permissions it holds, enabled %s",
  async (enabled) => {
    openPlugin(
      "cleanup",
      cleanupPlugin([PluginPermission.STORE, PluginPermission.READ_LIBRARIES], {
        enabled,
        kind: PluginKind.BUILT_IN,
        grantedPermissions: [PluginPermission.STORE, PluginPermission.READ_LIBRARIES],
      }),
    );

    const found6 = await plugin("cleanup");
    const held = "Always holds, as part of the Server: Keep its own data, Read Libraries";
    await expect(found6.findByText(held)).resolves.toBeDefined();
    expect(found6.queryByRole("checkbox")).toBeNull();
  },
);
