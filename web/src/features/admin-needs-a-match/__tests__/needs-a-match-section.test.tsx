import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { type ListUnmatchedItemsRequest, TitleKind } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { admin, fakeServer } from "@/test-server";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
const shows = { id: 2n, name: "Shows", kind: LibraryKind.SHOWS, folders: ["/srv/shows"] };
const alien = { itemId: 7n, path: "/srv/films/Alien.mkv", kind: TitleKind.MOVIE, title: "Alien" };
const dark = { itemId: 8n, path: "/srv/shows/Dark.mkv", kind: TitleKind.SHOW, title: "Dark" };

/** A fake Server whose `signedIn` User sees `libraries`, with Alien Unmatched in Films and Dark in Shows. */
function matchServer(signedIn: Permission[], libraries = [films, shows]) {
  return fakeServer(
    { signedIn, libraries },
    {
      MatchService: {
        listUnmatchedItems: ({ filter }) => ({
          items: [alien, dark].filter((item) =>
            item.path.startsWith(filter?.libraryId === 1n ? "/srv/films" : "/srv/shows"),
          ),
        }),
      },
    },
  ).transport;
}

describe("where Needs a Match is", () => {
  test("a User who manages Libraries but isn't an Admin sees only Media past their own", async () => {
    openSettings("needs-a-match", matchServer([Permission.MANAGE_LIBRARIES], [films]));

    await expect(screen.findByRole("link", { name: "Alien" })).resolves.toBeDefined();
    const nav = within(screen.getByRole("navigation", { name: "Settings" }));
    expect(nav.getAllByRole("heading").map((heading) => heading.textContent)).toStrictEqual([
      "You",
      "Preferences",
      "Media",
    ]);
    expect(nav.getByRole("link", { name: "Needs a Match" })).toBeDefined();
  });

  test("it's under Media, before Activity, and searching Settings for unmatched finds it", async () => {
    openSettings("playback", fakeServer({ signedIn: admin }).transport);

    await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
    const nav = within(screen.getByRole("navigation", { name: "Settings" }));
    expect(nav.getAllByRole("heading").map((heading) => heading.textContent)).toStrictEqual([
      "You",
      "Preferences",
      "Media",
      "Activity",
      "Admin",
    ]);
    const links = nav.getAllByRole("link").map((link) => link.textContent);
    expect(
      links.slice(links.indexOf("Connections"), links.indexOf("Now Playing") + 1),
    ).toStrictEqual(["Connections", "Libraries", "Needs a Match", "Now Playing"]);
    fireEvent.change(screen.getByRole("combobox", { name: "Search settings" }), {
      target: { value: "unmatched" },
    });

    expect(screen.getAllByRole("option").map((option) => option.textContent)).toStrictEqual([
      "Needs a Match",
    ]);
  });
});

test("every Library's Items are asked for at once, not after the Library before's", async () => {
  const asked: ListUnmatchedItemsRequest[] = [];
  openSettings(
    "needs-a-match",
    fakeServer(
      { signedIn: admin, libraries: [films, shows] },
      {
        MatchService: {
          listUnmatchedItems: async (request) => {
            asked.push(request);
            // Films never answers, so Shows is asked only if it isn't waiting on Films.
            await new Promise(() => {});
            return {};
          },
        },
      },
    ).transport,
  );

  await waitFor(() => {
    expect(new Set(asked.map((request) => request.filter?.libraryId))).toStrictEqual(
      new Set([films.id, shows.id]),
    );
  });
});

describe("the Library picker", () => {
  test("isn't there with one Library", async () => {
    openSettings("needs-a-match", matchServer(admin, [films]));

    await expect(screen.findByRole("link", { name: "Alien" })).resolves.toBeDefined();
    expect(screen.queryByRole("combobox", { name: "Library" })).toBeNull();
    expect(screen.queryByRole("radiogroup", { name: "Library" })).toBeNull();
  });

  test("has a segment for all and one for each Library, each with how many", async () => {
    openSettings("needs-a-match", matchServer(admin));

    await screen.findByRole("link", { name: "Dark" });

    const segments = within(screen.getByRole("radiogroup", { name: "Library" }));
    expect(
      segments.getAllByRole("radio").map((segment) => segment.parentElement?.textContent),
    ).toStrictEqual(["All 2", "Films 1", "Shows 1"]);
  });

  test("is a dropdown past 4 Libraries", async () => {
    const more = [
      { ...films, id: 3n },
      { ...films, id: 4n },
      { ...films, id: 5n },
    ];
    openSettings("needs-a-match", matchServer(admin, [films, shows, ...more]));

    await screen.findAllByRole("link", { name: "Dark" });

    expect(screen.getByRole("combobox", { name: "Library" })).toBeDefined();
    expect(screen.queryByRole("radiogroup", { name: "Library" })).toBeNull();
  });

  test("shows one Library's Items, and another section forgets which", async () => {
    const router = openSettings("needs-a-match", matchServer(admin));
    await expect(screen.findByRole("link", { name: "Dark" })).resolves.toBeDefined();
    expect(screen.getByRole("link", { name: "Alien" })).toBeDefined();

    fireEvent.click(screen.getByRole("radio", { name: "Shows 1" }));

    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "Alien" })).toBeNull();
    });
    expect(screen.getByRole("link", { name: "Dark" })).toBeDefined();
    expect(router.state.location.search.library).toBe("2");
    fireEvent.click(screen.getByRole("link", { name: "Playback" }));
    await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
    expect(router.state.location.search.library).toBeUndefined();
  });

  test("closing Settings forgets which", async () => {
    const router = openSettings("needs-a-match", matchServer(admin));
    await expect(screen.findByRole("link", { name: "Dark" })).resolves.toBeDefined();
    fireEvent.click(screen.getByRole("radio", { name: "Shows 1" }));
    await waitFor(() => {
      expect(router.state.location.search.library).toBe("2");
    });

    fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));

    await waitFor(() => {
      expect(router.state.location.search.settings).toBeUndefined();
    });
    expect(router.state.location.search.library).toBeUndefined();
  });

  test("says when the picked Library isn't on the Server", async () => {
    openApp("/code?settings=needs-a-match&library=9", matchServer(admin));

    await expect(screen.findByText("That Library isn’t on the Server.")).resolves.toBeDefined();
    expect(screen.queryByText("Everything is Matched")).toBeNull();
    expect(screen.getByRole("combobox", { name: "Library" })).toBeDefined();
  });

  test("keeps the picked Library through a file's screen and back", async () => {
    const router = openSettings("needs-a-match", matchServer(admin));
    await screen.findByRole("link", { name: "Dark" });
    fireEvent.click(screen.getByRole("radio", { name: "Shows 1" }));
    fireEvent.click(await screen.findByRole("link", { name: "Dark" }));
    await expect(
      within(await screen.findByRole("navigation", { name: "Breadcrumbs" })).findByRole("link", {
        name: "Needs a Match",
      }),
    ).resolves.toBeDefined();
    expect(router.state.location.search.item).toBe("8");
    expect(router.state.location.search.library).toBe("2");

    fireEvent.click(
      within(screen.getByRole("navigation", { name: "Breadcrumbs" })).getByRole("link", {
        name: "Needs a Match",
      }),
    );

    await waitFor(() => {
      expect(router.state.location.search.item).toBeUndefined();
    });
    expect(router.state.location.search.library).toBe("2");
  });
});
