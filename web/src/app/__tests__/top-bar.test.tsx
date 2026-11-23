import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const libraries = [
  { id: 1n, name: "Movies", kind: LibraryKind.MOVIES },
  { id: 2n, name: "TV Shows", kind: LibraryKind.SHOWS },
];

test("the top bar links each Library after Home, marking the one open", async () => {
  const router = openApp("/code", fakeServer({ signedIn: newUser, libraries }));

  const main = await screen.findByRole("navigation", { name: "Main" });
  await within(main).findByRole("link", { name: "TV Shows" });
  const links = within(main).getAllByRole("link");
  expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toStrictEqual([
    ["Home", "/"],
    ["Movies", "/libraries/1"],
    ["TV Shows", "/libraries/2"],
  ]);

  await router.navigate({ to: "/libraries/$libraryId", params: { libraryId: "2" } });
  expect(
    within(main)
      .getAllByRole("link", { current: "page" })
      .map((link) => link.textContent),
  ).toStrictEqual(["TV Shows"]);
});

test("on a phone the Libraries menu links each Library and is named by the one open", async () => {
  const router = openApp("/code", fakeServer({ signedIn: newUser, libraries }));

  fireEvent.click(await screen.findByRole("button", { name: "Libraries" }));
  const items = await screen.findAllByRole("menuitem");
  expect(items.map((item) => [item.textContent, item.getAttribute("href")])).toStrictEqual([
    ["Movies", "/libraries/1"],
    ["TV Shows", "/libraries/2"],
  ]);

  fireEvent.click(screen.getByRole("menuitem", { name: "TV Shows" }));
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/2");
  });
  await screen.findByRole("button", { name: "TV Shows" });
});
