import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError, type Interceptor, createRouterTransport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { LibraryKind, type LibrarySchema, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

/** The origins whose Servers fail to list their Libraries. */
const failing = new Set<string>();

afterEach(() => {
  cleanup();
  failing.clear();
  localStorage.clear();
  vi.unstubAllGlobals();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});
const cabin = create(LinkedServerSchema, {
  id: "cabin",
  name: "Cabin",
  addresses: ["https://cabin.example"],
});

const libraries: Record<string, MessageInitShape<typeof LibrarySchema>[]> = {
  "https://basement.example": [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "Series", kind: LibraryKind.SHOWS },
  ],
  "https://cabin.example": [{ id: 1n, name: "Movies", kind: LibraryKind.MOVIES }],
};

/** A fake Server at `origin` that signs the Hub's Account in as Ada and lists its Libraries. */
function fakeServerAt(origin: string, interceptors: Interceptor[]) {
  return createRouterTransport(
    (router) => {
      router.service(ServerService, { getServerInfo: () => ({ version: "0.3.0" }) });
      router.service(SignInService, {
        signInWithHubToken: () => ({ token: "session" }),
        whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
      });
      router.service(LibraryService, {
        listLibraries: () => {
          if (failing.has(origin)) {
            throw new ConnectError("failed", Code.Internal);
          }
          return { libraries: libraries[origin] ?? [] };
        },
      });
    },
    { transport: { interceptors } },
  );
}

/** Opens `path` at the Hub for an Account on `servers`. */
function openHub(path: string, servers = [basement]) {
  const { transport } = accountHub({ servers });
  return openApp(path, transport, undefined, fakeServerAt);
}

function itemsOn(server: string) {
  return within(screen.getByRole("group", { name: server }))
    .getAllByRole("menuitem")
    .map((item) => [item.textContent, item.getAttribute("href")]);
}

function linksIn(element: HTMLElement) {
  return within(element)
    .getAllByRole("link")
    .map((link) => [link.textContent, link.getAttribute("href")]);
}

test("with one Server the Hub's bar links Home and each of its Libraries on it", async () => {
  openHub("/");

  const main = await screen.findByRole("navigation", { name: "Main" });
  await within(main).findByRole("link", { name: "Series" });
  expect(linksIn(main)).toStrictEqual([
    ["Home", "/"],
    ["Films", "/libraries/1?server=basement"],
    ["Series", "/libraries/2?server=basement"],
  ]);
});

test("with two Servers the Hub's bar links Home and a menu of each Server's Libraries under its name", async () => {
  openHub("/", [basement, cabin]);

  const main = await screen.findByRole("navigation", { name: "Main" });
  fireEvent.click(await screen.findByRole("button", { name: "Libraries" }));
  await vi.waitFor(() => {
    expect(screen.getAllByRole("group")).toHaveLength(2);
  });
  expect(screen.getAllByRole("group").map((group) => group.firstChild?.textContent)).toStrictEqual([
    "Basement",
    "Cabin",
  ]);
  expect(itemsOn("Basement")).toStrictEqual([
    ["Films", "/libraries/1?server=basement"],
    ["Series", "/libraries/2?server=basement"],
  ]);
  expect(itemsOn("Cabin")).toStrictEqual([["Movies", "/libraries/1?server=cabin"]]);
  expect(linksIn(main)).toStrictEqual([["Home", "/"]]);
});

test("a second Server whose Libraries fail still folds the first's into the menu", async () => {
  failing.add("https://cabin.example");
  openHub("/", [basement, cabin]);

  fireEvent.click(await screen.findByRole("button", { name: "Libraries" }));
  await screen.findByRole("group", { name: "Basement" });
  expect(itemsOn("Basement")).toStrictEqual([
    ["Films", "/libraries/1?server=basement"],
    ["Series", "/libraries/2?server=basement"],
  ]);
  expect(screen.queryByRole("group", { name: "Cabin" })).toBeNull();
});

test("the Hub's menu has the Watchlist, the Account's Settings and signing out, and the bar no Inbox", async () => {
  const router = openHub("/");

  fireEvent.click(await screen.findByRole("button", { name: "ada@example.com’s menu" }));
  const items = await screen.findAllByRole("menuitem");
  expect(items.map((item) => item.textContent)).toStrictEqual([
    "Watchlist",
    "Settings",
    "Sign out",
  ]);
  expect(screen.queryByRole("button", { name: /Notifications/u })).toBeNull();

  fireEvent.click(screen.getByRole("menuitem", { name: "Settings" }));
  await vi.waitFor(() => {
    expect(router.state.location.search).toMatchObject({ settings: "account" });
  });
});

test("a Server's page at the Hub keeps the Hub's bar", async () => {
  openHub("/libraries/2?server=basement", [basement, cabin]);

  await screen.findByRole("button", { name: "Search your Servers" });
  fireEvent.click(await screen.findByRole("button", { name: "Series" }));
  await vi.waitFor(() => {
    expect(screen.getAllByRole("group")).toHaveLength(2);
  });
  expect(linksIn(screen.getByRole("navigation", { name: "Main" }))).toStrictEqual([["Home", "/"]]);
});

test("the Watchlist and the Hub's own pages sit under the bar, out of the signed-out frame", async () => {
  const router = openHub("/watchlist");

  await screen.findByRole("navigation", { name: "Main" });
  await router.navigate({ to: "/link-server/$code", params: { code: "ABC" } });
  await screen.findByRole("heading", { name: "Link a Server" });
  expect(screen.getByRole("navigation", { name: "Main" })).toBeDefined();
  expect(document.querySelector("canvas")).toBeNull();
});
