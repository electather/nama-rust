import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { TitleKind, UnmatchedReason } from "@/gen/nama/v1/match_pb";
import { eventFeed } from "@/test-app";
import { admin, fakeServer } from "@/test-server";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
const shows = { id: 2n, name: "Shows", kind: LibraryKind.SHOWS, folders: ["/srv/shows"] };
const alien = {
  itemId: 7n,
  path: "/srv/films/Alien.1979.mkv",
  kind: TitleKind.MOVIE,
  title: "Alien",
  year: 1979,
  reason: UnmatchedReason.NO_TITLE_FITS,
};
const severance = {
  itemId: 8n,
  path: "/srv/shows/Severance/S01E02.mkv",
  kind: TitleKind.SHOW,
  title: "Severance",
  season: 1,
  episode: 2,
  reason: UnmatchedReason.NOT_IN_SHOW,
};

/** A fake Server with Films listing Alien and Shows listing an Episode of Severance. */
function listing() {
  return fakeServer(
    { signedIn: admin, libraries: [films, shows] },
    {
      MatchService: {
        listUnmatchedItems: ({ filter }) => ({
          items: filter?.libraryId === 1n ? [alien] : [severance],
        }),
      },
    },
  ).transport;
}

describe("the list", () => {
  test("says how many files there are over each Library's own inset list", async () => {
    openSettings("needs-a-match", listing());

    await expect(
      screen.findByText(
        "The Server couldn’t tell what these 2 files are. Open one to pick its title.",
      ),
    ).resolves.toBeDefined();
    const movies = screen.getByRole("region", { name: "Films" });
    expect(within(movies).getByRole("link", { name: "Alien (1979)" })).toBeDefined();
    expect(within(movies).getByText("No title fits this name")).toBeDefined();
    const episodes = screen.getByRole("region", { name: "Shows" });
    expect(
      within(episodes).getByRole("link", { name: "Severance · Season 1, Episode 2" }),
    ).toBeDefined();
    expect(within(episodes).getByText("This Episode isn’t in Season 1")).toBeDefined();
  });

  test("shows a row's name and why, with no path and no buttons", async () => {
    openSettings("needs-a-match", listing());

    await screen.findByRole("link", { name: "Alien (1979)" });

    expect(screen.queryByText(alien.path)).toBeNull();
    const list = screen.getByRole("region", { name: "Films" });
    expect(within(list).queryAllByRole("button")).toStrictEqual([]);
  });

  test("names the one file when there's only one", async () => {
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [films] },
        { MatchService: { listUnmatchedItems: () => ({ items: [alien] }) } },
      ).transport,
    );

    await expect(
      screen.findByText("The Server couldn’t tell what this file is. Open one to pick its title."),
    ).resolves.toBeDefined();
  });

  test("names an Item parsed as Season 0 a Special", async () => {
    const special = { ...severance, season: 0, episode: 2 };
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [shows] },
        { MatchService: { listUnmatchedItems: () => ({ items: [special] }) } },
      ).transport,
    );

    await expect(
      screen.findByRole("link", { name: "Severance · Specials, Episode 2" }),
    ).resolves.toBeDefined();
  });

  test("lists the files only a rename fixes under their Library", async () => {
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [films] },
        { MatchService: { listUnmatchedItems: () => ({ unparsedPaths: ["/srv/films/x.mkv"] }) } },
      ).transport,
    );

    await expect(screen.findByText("/srv/films/x.mkv")).resolves.toBeDefined();
    expect(screen.getByText("Files to rename")).toBeDefined();
  });

  test("says when everything is Matched", async () => {
    openSettings("needs-a-match", fakeServer({ signedIn: admin, libraries: [films] }).transport);

    await expect(screen.findByText("Everything is Matched")).resolves.toBeDefined();
  });
});

describe("as Matching goes on", () => {
  test("shows why once Matching finishes", async () => {
    const events = eventFeed();
    let reason: UnmatchedReason = UnmatchedReason.UNSPECIFIED;
    let listed = 0;
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [films], events: events.stream },
        {
          MatchService: {
            listUnmatchedItems: () => {
              listed += 1;
              return { items: [{ ...alien, reason }] };
            },
          },
        },
      ).transport,
    );
    await screen.findByText("Not Matched yet");
    // The scan finished before Matching did. Its Event opens the stream, and the catch-up refetch
    // that follows has to land first, or it would be the one to show the reason.
    events.send({ event: { case: "scanFinished", value: { libraryId: 1n } } });
    await waitFor(() => {
      expect(listed).toBe(3);
    });

    reason = UnmatchedReason.NO_TITLE_FITS;
    events.send({ event: { case: "matchingFinished", value: { libraryId: 1n, unmatched: 1n } } });

    await expect(screen.findByText("No title fits this name")).resolves.toBeDefined();
  });

  test("an Item Matched elsewhere leaves the list when its Event arrives", async () => {
    const events = eventFeed();
    const items = [alien];
    let unmatched = (): object => ({ items });
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [films], events: events.stream },
        { MatchService: { listUnmatchedItems: () => unmatched() } },
      ).transport,
    );
    await screen.findByRole("link", { name: "Alien (1979)" });

    // Matched automatically, or by another Admin.
    // From now on a refetch never answers, so only the Event can take the Item off.
    unmatched = async () => {
      await new Promise(() => {});
      return {};
    };
    events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId: 7n } } });

    await expect(screen.findByText("Everything is Matched")).resolves.toBeDefined();
  });

  test("a row's file opens when pressed, whatever of the row it hits", async () => {
    openSettings("needs-a-match", listing());

    fireEvent.click(await screen.findByRole("link", { name: "Alien (1979)" }));

    await expect(
      screen.findByRole("heading", { name: "Alien (1979)", level: 3 }),
    ).resolves.toBeDefined();
  });
});
