import { fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import {
  type DeletePluginConnectionRequest,
  PluginConnectionState,
  type StartPluginConnectionRequest,
} from "@/gen/nama/v1/user_pb";

import { fakeServer, newUser, openApp } from "./test-app";

const link = "https://t.me/nama_bot?start=one-time-code";

afterEach(() => {
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

test("a User connects Telegram on Settings, waits for Start, then sees it Connected", async () => {
  const opened = vi.spyOn(window, "open").mockReturnValue(null);
  const state: { now: PluginConnectionState } = { now: PluginConnectionState.NOT_CONNECTED };
  const { transport, started } = telegramIn(state);
  openApp("/settings", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Connect Telegram" }));

  expect(await screen.findByText("Open Telegram and press Start.")).toBeDefined();
  expect(screen.getByRole("link", { name: "Open Telegram" }).getAttribute("href")).toBe(link);
  expect(opened).toHaveBeenCalledWith(link, "_blank", "noopener");
  expect(started).toMatchObject([{ pluginId: "telegram" }]);

  state.now = PluginConnectionState.CONNECTED;

  const connections = screen.getByRole("region", { name: "Connections" });
  expect(await within(connections).findByText("Connected")).toBeDefined();
  expect(within(connections).getByRole("button", { name: "Disconnect Telegram" })).toBeDefined();
});

test("Disconnect deletes the User's Telegram connection", async () => {
  const { transport, deleted } = telegramIn({ now: PluginConnectionState.CONNECTED });
  openApp("/settings", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Disconnect Telegram" }));

  expect(await screen.findByRole("button", { name: "Connect Telegram" })).toBeDefined();
  expect(deleted).toMatchObject([{ pluginId: "telegram" }]);
});

test("Settings shows no Connections while no Plugin connects Users", async () => {
  openApp("/settings", fakeServer({ signedIn: newUser }));

  await screen.findByRole("heading", { name: "Devices" });

  expect(screen.queryByRole("heading", { name: "Connections" })).toBeNull();
});
