import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type ServerState, fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const manageSettings = [...newUser, Permission.MANAGE_SETTINGS];

function openServer(serverService: NonNullable<ServerState["serverService"]>) {
  openSettings("server", fakeServer({ signedIn: manageSettings, serverService }));
}

/** Where Linking stands, as the fake Server answers it. */
interface Shown {
  code?: string;
  url?: string;
  account?: string;
  publicAddress?: string;
  reachable?: boolean;
  names?: string[];
}

const waiting = { code: "BDFH-JKLM", url: "https://hub.example" };

test("without the Hub Plugin, the Server pane has no Hub section", async () => {
  openServer({});

  await screen.findByLabelText("Server URL");
  expect(screen.queryByRole("heading", { name: "Hub" })).toBeNull();
});

test("starting to Link shows the code to enter on the Hub, and cancelling stops showing it", async () => {
  let link: Shown = {};
  const calls: string[] = [];
  openServer({
    getServerLink: () => ({ link }),
    startServerLink: () => {
      calls.push("start");
      link = waiting;
      return { link };
    },
    unlinkServer: () => {
      calls.push("unlink");
      link = {};
      return {};
    },
  });

  await expect(screen.findByText("Not Linked")).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Link to the Hub" }));

  await expect(screen.findByText("BDFH-JKLM")).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "the Hub" }).getAttribute("href")).toBe(
    "https://hub.example",
  );
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await expect(screen.findByText("Not Linked")).resolves.toBeDefined();
  expect(calls).toStrictEqual(["start", "unlink"]);
});

test("a Linked Server names its Account, and Unlinking it says it isn't Linked", async () => {
  let link: Shown = { account: "ada@example.com" };
  openServer({
    getServerLink: () => ({ link }),
    unlinkServer: () => {
      link = {};
      return {};
    },
  });

  await expect(screen.findByText("ada@example.com")).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Unlink" }));

  await expect(screen.findByText("Not Linked")).resolves.toBeDefined();
});

test("a Linked Server says whether the Hub reached it from outside the LAN", async () => {
  let link: Shown = {
    account: "ada@example.com",
    publicAddress: "http://203.0.113.7:7980",
    reachable: true,
  };
  openServer({ getServerLink: () => ({ link }) });

  await expect(
    screen.findByText("Reachable from outside the LAN at http://203.0.113.7:7980."),
  ).resolves.toBeDefined();
  cleanup();
  link = { ...link, reachable: false };
  openServer({ getServerLink: () => ({ link }) });

  await expect(
    screen.findByText(
      "The Hub couldn’t reach the Server at http://203.0.113.7:7980 from outside the LAN. Check that your router forwards that port to the Server.",
    ),
  ).resolves.toBeDefined();
});

test("a Linked Server lists the names the Hub gave it, its own first", async () => {
  const names = ["abc.namadirect.net", "192-168-1-10.abc.namadirect.net"];
  openServer({ getServerLink: () => ({ link: { account: "ada@example.com", names } }) });

  const list = await screen.findByRole("list", { name: "The Hub’s names for the Server:" });

  expect(Array.from(list.children, (item) => item.textContent)).toStrictEqual(names);
});

test("a Hub the Server can't reach is said, and the Server stays as it was", async () => {
  openServer({
    getServerLink: () => ({ link: {} }),
    startServerLink: () => {
      throw new ConnectError("the Plugin that links the Server failed", Code.Unavailable);
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  const said = await screen.findByText(
    "The Server couldn’t reach the Hub. Try again; the Hub Plugin’s page under Plugins says why.",
  );
  expect(said.getAttribute("role")).toBe("alert");
  expect(screen.getByText("Not Linked")).toBeDefined();
});
