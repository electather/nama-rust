import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type ServerState, fakeServer, newUser } from "@/test-app";
import { refused } from "@/test-refusal";
import { openSettings } from "@/test-settings";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

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

test("when the config names no Hub, the Server pane has no Hub section", async () => {
  openServer({});

  await screen.findByLabelText("Server URL");
  expect(screen.queryByRole("heading", { name: "Hub" })).toBeNull();
});

test("linking to the Hub opens its approval, which comes back to this page", async () => {
  // Going to another site would end the test's page, so where it went is only noted.
  const went: string[] = [];
  vi.spyOn(globalThis.location, "assign").mockImplementation((url) => {
    went.push(String(url));
  });
  const returnUrls: string[] = [];
  openServer({
    getServerLink: () => ({ link: {} }),
    startServerLink: ({ returnUrl }) => {
      returnUrls.push(returnUrl);
      return { link: { code: "BDFH-JKLM", url: "https://hub.example/link-server/BDFH-JKLM" } };
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  await vi.waitFor(() => {
    expect(went).toStrictEqual(["https://hub.example/link-server/BDFH-JKLM"]);
  });
  expect(returnUrls).toStrictEqual([globalThis.location.href]);
});

test("a Server opened off its own addresses says to show a code instead", async () => {
  openServer({
    getServerLink: () => ({ link: {} }),
    startServerLink: () => {
      throw refused(Code.InvalidArgument, "open the Server at its Server URL", {
        reason: RefusalReason.NOT_A_SERVER_ADDRESS,
        field: "return_url",
      });
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  await expect(
    screen.findByText(
      "Open the Server at its Server URL to Link it this way, or show a code instead.",
    ),
  ).resolves.toBeDefined();
});

test("a Server whose config names no Hub says to name one", async () => {
  openServer({
    getServerLink: () => ({ link: {} }),
    startServerLink: () => {
      throw refused(Code.FailedPrecondition, "the Server's config names no Hub", {
        reason: RefusalReason.NO_SERVER_LINKER,
      });
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  await expect(
    screen.findByText(
      "The Server’s config names no Hub, so it can’t be Linked. Name one in the config.",
    ),
  ).resolves.toBeDefined();
});

test("showing a code instead shows the code to enter on the Hub, and cancelling stops showing it", async () => {
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
  fireEvent.click(screen.getByRole("button", { name: "Show a code instead" }));

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
      throw new ConnectError("the Hub didn’t answer", Code.Unavailable);
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  const said = await screen.findByText("The Server couldn’t reach the Hub. Try again in a moment.");
  expect(said.getAttribute("role")).toBe("alert");
  expect(screen.getByText("Not Linked")).toBeDefined();
});
