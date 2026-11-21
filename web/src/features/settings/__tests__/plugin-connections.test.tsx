import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import {
  type DeletePluginConnectionRequest,
  PluginConnectionState,
  type StartPluginConnectionRequest,
} from "@/gen/nama/v1/user_pb";
import { admin, fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

const link = "https://t.me/nama_bot?start=one-time-code";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** A fake Server whose Telegram connection is in `state`, which the test may change. */
function telegramIn(state: { now: PluginConnectionState }) {
  const started: StartPluginConnectionRequest[] = [];
  const deleted: DeletePluginConnectionRequest[] = [];
  const transport = fakeServer(
    { signedIn: newUser },
    {},
    {
      listPluginConnections: () => ({ connections: [{ pluginId: "telegram", state: state.now }] }),
      startPluginConnection: (request) => {
        started.push(request);
        state.now = PluginConnectionState.WAITING;
        return { url: link };
      },
      deletePluginConnection: (request) => {
        deleted.push(request);
        state.now = PluginConnectionState.NOT_CONNECTED;
        return {};
      },
    },
  );
  return { transport, started, deleted };
}

test("a User connects Telegram on Settings, waits for it, then sees it Connected", async () => {
  const opened = vi.spyOn(globalThis, "open").mockReturnValue(null);
  const state: { now: PluginConnectionState } = { now: PluginConnectionState.NOT_CONNECTED };
  const { transport, started } = telegramIn(state);
  openSettings("connections", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Connect in Telegram" }));

  await expect(
    screen.findByText("Finish in Telegram. This updates on its own."),
  ).resolves.toBeDefined();
  expect(screen.getByText("Waiting for you")).toBeDefined();
  expect(screen.getByRole("link", { name: "Open Telegram again" }).getAttribute("href")).toBe(link);
  expect(opened).toHaveBeenCalledWith(link, "_blank", "noopener");
  expect(started).toMatchObject([{ pluginId: "telegram" }]);

  state.now = PluginConnectionState.CONNECTED;

  const connections = screen.getByRole("list", { name: "Plugins" });
  await expect(within(connections).findByText("Connected")).resolves.toBeDefined();
  expect(within(connections).getByRole("button", { name: "Disconnect" })).toBeDefined();
});

test("cancelling a link that waits goes back to not connected", async () => {
  const { transport, deleted } = telegramIn({ now: PluginConnectionState.WAITING });
  openSettings("connections", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Cancel connecting Telegram" }));

  await expect(screen.findByRole("button", { name: "Connect in Telegram" })).resolves.toBeDefined();
  expect(deleted).toMatchObject([{ pluginId: "telegram" }]);
});

test("disconnect asks first, then deletes the User's Telegram connection", async () => {
  const { transport, deleted } = telegramIn({ now: PluginConnectionState.CONNECTED });
  openSettings("connections", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Disconnect" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Disconnect Telegram?" });
  expect(deleted).toStrictEqual([]);
  fireEvent.click(within(dialog).getByRole("button", { name: "Disconnect" }));

  await expect(screen.findByRole("button", { name: "Connect in Telegram" })).resolves.toBeDefined();
  expect(deleted).toMatchObject([{ pluginId: "telegram" }]);
});

test("while no Plugin connects Users, the pane says so, and to ask an Admin", async () => {
  openSettings("connections", fakeServer({ signedIn: newUser }));

  await expect(
    screen.findByText("No Plugins on this Server can message you yet."),
  ).resolves.toBeDefined();
  expect(screen.getByText("Ask an Admin to add one.")).toBeDefined();
  expect(screen.queryByRole("link", { name: "See Plugins" })).toBeNull();
});

test("an Admin with no Plugin that connects Users is sent to the Plugins section", async () => {
  openSettings("connections", fakeServer({ signedIn: admin }));

  const see = await screen.findByRole("link", { name: "See Plugins" });
  fireEvent.click(see);

  await expect(screen.findByRole("heading", { name: "Plugins" })).resolves.toBeDefined();
});
