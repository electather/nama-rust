import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SettingKind } from "@/gen/nama/v1/plugin_pb";
import { WatchSyncLinkState } from "@/gen/nama/v1/user_pb";
import { fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

type Users = Parameters<typeof fakeServer>[2];

/** Trakt as a Watch sync, its account linked when `linked`. */
function traktSettings(linked: boolean) {
  return {
    pluginId: "trakt",
    name: "Trakt",
    watchSync: true,
    settings: [
      {
        key: "token",
        label: "Token",
        description: "",
        kind: SettingKind.SECRET,
        secretSet: linked,
      },
    ],
  };
}

const started = {
  userCode: "ABCD1234",
  verificationUrl: "https://trakt.tv/activate",
  expiresInSeconds: 600,
  intervalSeconds: 1,
  deviceCode: "device-1",
};

/** Opens Settings with Trakt, linked when the fake Server's `linked` says. */
function openWithTrakt(users: Users, linked: () => boolean = () => false) {
  openSettings(
    "connections",
    fakeServer(
      { signedIn: newUser },
      {},
      { listPersonalSettings: () => ({ plugins: [traktSettings(linked())] }), ...users },
    ),
  );
}

const card = async () => within(await screen.findByRole("listitem", { name: "Trakt" }));

/** Long enough for a poll a second after the code shows. */
const POLLED = { timeout: 3000 };

test("linking shows the code and where to enter it, then that the account is linked", async () => {
  let linked = false;
  let polled = "";
  openWithTrakt(
    {
      startWatchSyncLink: () => started,
      pollWatchSyncLink: ({ deviceCode }) => {
        polled = deviceCode;
        linked = true;
        return { state: WatchSyncLinkState.LINKED };
      },
    },
    () => linked,
  );

  const trakt = await card();
  expect(trakt.getByText("Not linked")).toBeDefined();
  expect(trakt.queryByLabelText("Token")).toBeNull();
  fireEvent.click(trakt.getByRole("button", { name: "Link your account" }));

  await expect(trakt.findByText("ABCD1234")).resolves.toBeDefined();
  expect(trakt.getByRole("link", { name: "Trakt" }).getAttribute("href")).toBe(
    "https://trakt.tv/activate",
  );
  // Not asked before the interval the tracker allows is up.
  expect(polled).toBe("");
  expect(trakt.getByText(/It works for 10 minutes\./u)).toBeDefined();
  await expect(trakt.findByText("Your account is linked", {}, POLLED)).resolves.toBeDefined();
  expect(polled).toBe("device-1");
});

test("an expired code says so plainly and offers a new one", async () => {
  openWithTrakt({
    startWatchSyncLink: () => started,
    pollWatchSyncLink: () => ({ state: WatchSyncLinkState.EXPIRED }),
  });

  const trakt = await card();
  fireEvent.click(trakt.getByRole("button", { name: "Link your account" }));

  await expect(
    trakt.findByText(
      "The code expired before it was entered. Get a new one to try again.",
      {},
      POLLED,
    ),
  ).resolves.toBeDefined();
  expect(trakt.getByRole("button", { name: "Get a new code" })).toBeDefined();
});

test("unlinking forgets the account", async () => {
  let linked = true;
  let unlinked = "";
  openWithTrakt(
    {
      deletePersonalSettings: ({ pluginId }) => {
        unlinked = pluginId;
        linked = false;
        return {};
      },
    },
    () => linked,
  );

  const trakt = await card();
  fireEvent.click(trakt.getByRole("button", { name: "Unlink" }));

  await expect(trakt.findByText("Not linked")).resolves.toBeDefined();
  expect(unlinked).toBe("trakt");
});

test("a tracker the Server can't reach is said, and the account stays as it was", async () => {
  openWithTrakt({
    startWatchSyncLink: () => {
      throw new ConnectError("the Plugin couldn't ask its tracker", Code.Unavailable);
    },
  });

  const trakt = await card();
  fireEvent.click(trakt.getByRole("button", { name: "Link your account" }));

  const said = await screen.findByText("The Server couldn’t reach Trakt. Try again in a moment.");
  expect(said.getAttribute("role")).toBe("alert");
  expect(screen.getByText("Not linked")).toBeDefined();
});
