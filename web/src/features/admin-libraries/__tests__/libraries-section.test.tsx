import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer } from "@/test-server";
import { findSettingsLink, openLibraries, openSettings } from "@/test-settings";

afterEach(cleanup);

type Library = MessageInitShape<typeof LibrarySchema>;

const films: Library = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

/** A fake Server with `libraries`, kept as they're changed or deleted, for `signedIn`. */
function librariesServer(libraries: Library[], signedIn: Permission[] = admin) {
  const { server, transport } = fakeServer(
    { signedIn, libraries },
    {
      LibraryService: {
        updateLibrary: ({ id, folders }) => {
          server.libraries = server.libraries.map((library) =>
            library.id === id ? { ...library, folders } : library,
          );
          return { library: server.libraries.find((library) => library.id === id) };
        },
        deleteLibrary: ({ id }) => {
          server.libraries = server.libraries.filter((library) => library.id !== id);
          return {};
        },
      },
    },
  );
  return transport;
}

/** The Settings breadcrumbs. */
function trail() {
  return screen.getByRole("navigation", { name: "Breadcrumbs" });
}

/** The words the Settings breadcrumbs say. */
function crumbs() {
  return trail().textContent;
}

describe("where Libraries is", () => {
  test("a User who manages Libraries but isn't an Admin sees only the Media group", async () => {
    openLibraries(librariesServer([films], [Permission.MANAGE_LIBRARIES]));

    await findSettingsLink("Films");
    const nav = within(screen.getByRole("navigation", { name: "Settings" }));
    const headings = nav.getAllByRole("heading").map((heading) => heading.textContent);
    expect(headings).toStrictEqual(["You", "Preferences", "Media"]);
    expect(
      nav
        .getAllByRole("link")
        .slice(-2)
        .map((link) => link.textContent),
    ).toStrictEqual(["Libraries", "Needs a Match"]);
  });

  test("a User who doesn't manage Libraries has no Libraries section", async () => {
    openSettings("libraries", fakeServer({ signedIn: [] }).transport);

    await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
    expect(screen.queryByRole("link", { name: "Libraries" })).toBeNull();
  });

  test("searching Settings for recognisers finds it", async () => {
    openSettings("playback", fakeServer({ signedIn: admin }).transport);
    await screen.findByRole("heading", { name: "Playback" });

    fireEvent.change(screen.getByRole("combobox", { name: "Search settings" }), {
      target: { value: "recognisers" },
    });

    expect(screen.getAllByRole("option").map((option) => option.textContent)).toStrictEqual([
      "Libraries",
    ]);
  });
});

test("with no Libraries, the list says so", async () => {
  openLibraries(librariesServer([]));

  await expect(screen.findByText("No Libraries yet.")).resolves.toBeDefined();
});

test("new Library opens the form in place of the list, and its crumb goes back", async () => {
  openLibraries(librariesServer([films]));

  fireEvent.click(await screen.findByRole("link", { name: "New Library" }));
  await screen.findByRole("button", { name: "Create Library" });
  expect(crumbs()).toBe("LibrariesNew Library");

  fireEvent.click(within(trail()).getByRole("link", { name: "Libraries" }));
  await expect(findSettingsLink("Films")).resolves.toBeDefined();
  expect(crumbs()).toBe("Libraries");
});

test("a Library's row opens it in place of the list, under crumbs naming it", async () => {
  openLibraries(librariesServer([films]));

  fireEvent.click(await findSettingsLink("Films"));

  const title = await screen.findByRole("heading", { name: "Films" });
  expect(document.activeElement).toBe(title);
  expect(crumbs()).toBe("LibrariesFilms");
});

test("closing Settings with a Library open leaves no Library in the page's address", async () => {
  const router = openLibraries(librariesServer([films]), "1");

  fireEvent.click(await screen.findByRole("button", { name: "Close Settings" }));

  await waitFor(() => {
    expect(router.state.location.searchStr).toBe("");
  });
});

test("once a change to the folders is saved, it says to scan, beside Scan", async () => {
  openLibraries(librariesServer([{ ...films }]), "1");
  const folders = await screen.findByRole("textbox", { name: "Folders" });
  expect(screen.queryByText(/Folders saved/u)).toBeNull();

  fireEvent.change(folders, { target: { value: "/srv/films\n/srv/more films" } });
  fireEvent.blur(folders);

  await expect(
    screen.findByText("Folders saved. Scan the Library to pick them up."),
  ).resolves.toBeDefined();
  expect(screen.getByRole("button", { name: "Scan Films" })).toBeDefined();
});

test("deleting a Library goes back to the list, without it", async () => {
  openLibraries(librariesServer([{ ...films }, { ...films, id: 2n, name: "Shows" }]), "1");

  fireEvent.click(await screen.findByRole("button", { name: "Delete Films" }));
  const dialog = screen.getByRole("alertdialog", { name: "Delete Films?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  await expect(screen.findByRole("link", { name: "Shows" })).resolves.toBeDefined();
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: "Films" })).toBeNull();
  });
});
