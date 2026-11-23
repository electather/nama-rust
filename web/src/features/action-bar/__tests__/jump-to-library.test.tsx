import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const libraries = [
  { id: 1n, name: "Movies", kind: LibraryKind.MOVIES },
  { id: 2n, name: "Cartoons", kind: LibraryKind.SHOWS },
];

test("the action bar lists each Library by its kind, and its name finds it and opens it", async () => {
  const router = openApp("/code", fakeServer({ signedIn: newUser, libraries }));
  await screen.findByRole("button", { name: "Search this Server" });
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  const field = await screen.findByRole("combobox", { name: "Command menu" });

  const listed = await screen.findByRole("group", { name: "Libraries" });
  expect(
    Array.from(listed.querySelectorAll("[role=option]"), (row) => row.textContent),
  ).toStrictEqual(["MoviesA Library of Movies", "CartoonsA Library of Shows"]);

  fireEvent.change(field, { target: { value: "cartoons" } });
  await screen.findByRole("option", { name: /Cartoons/u, selected: true });
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/2");
  });
});
