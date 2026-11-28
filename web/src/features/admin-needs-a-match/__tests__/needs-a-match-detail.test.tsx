import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type SearchTitlesRequest,
  type SetMatchRequest,
  TitleKind,
  type UnmatchedItemSchema,
  UnmatchedReason,
} from "@/gen/nama/v1/match_pb";
import { openApp } from "@/test-app";
import { admin, fakeServer } from "@/test-server";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
const odyssey = {
  itemId: 7n,
  versionId: 70n,
  path: "/srv/films/The Odyssey (2026).mkv",
  kind: TitleKind.MOVIE,
  title: "The Odyssey",
  year: 2026,
  reason: UnmatchedReason.AMBIGUOUS,
  tiedTitles: [
    { title: "The Odyssey", year: 2026 },
    { title: "The Odyssey", year: 2026 },
  ],
};

const found = (title: { value: string; title: string; year: number; originalTitle?: string }) => ({
  id: { namespace: "tmdb", value: title.value },
  title: title.title,
  year: title.year,
  originalTitle: title.originalTitle,
});

/** What the fake Server was asked, and the Items it still lists. */
interface Asked {
  searched: SearchTitlesRequest[];
  matched: SetMatchRequest[];
  items: MessageInitShape<typeof UnmatchedItemSchema>[];
}

/** A fake Server listing `odyssey`, finding `candidates`, and dropping the Item once it's Matched. */
function odysseyServer(
  asked: Asked,
  candidates = [
    found({ value: "1", title: "The Odyssey", year: 2026 }),
    found({ value: "2", title: "The Odyssey", year: 2026, originalTitle: "Odyssey" }),
  ],
) {
  return fakeServer(
    { signedIn: admin, libraries: [films] },
    {
      MatchService: {
        listUnmatchedItems: () => ({ items: asked.items }),
        searchTitles: (request) => {
          asked.searched.push(request);
          return { candidates };
        },
        setMatch: (request) => {
          asked.matched.push(request);
          asked.items = [];
          return { matched: 1n };
        },
      },
    },
  ).transport;
}

function newAsked(): Asked {
  return { searched: [], matched: [], items: [odyssey] };
}

describe("a file's own screen", () => {
  test("opens from its row in the address, with the reason as a sentence, and its crumb returns", async () => {
    const router = openSettings("needs-a-match", odysseyServer(newAsked()));

    fireEvent.click(await screen.findByRole("link", { name: "The Odyssey (2026)" }));

    await expect(
      screen.findByText(
        "Two Movies called The Odyssey came out in 2026. Pick the one this file is.",
      ),
    ).resolves.toBeDefined();
    expect(router.state.location.search.item).toBe("7");
    expect(screen.getByRole("heading", { name: "Which Movie is it?" })).toBeDefined();
    fireEvent.click(
      within(screen.getByRole("navigation", { name: "Breadcrumbs" })).getByRole("link", {
        name: "Needs a Match",
      }),
    );
    await expect(screen.findByRole("link", { name: "The Odyssey (2026)" })).resolves.toBeDefined();
    expect(router.state.location.search.item).toBeUndefined();
  });

  test("is still there after a reload, which opens it straight from the address", async () => {
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(newAsked()));

    await expect(
      screen.findByRole("heading", { name: "The Odyssey (2026)", level: 3 }),
    ).resolves.toBeDefined();
  });

  test("shows the list for an Item that isn't listed", async () => {
    openApp("/code?settings=needs-a-match&item=99", odysseyServer(newAsked()));

    await expect(screen.findByRole("link", { name: "The Odyssey (2026)" })).resolves.toBeDefined();
  });

  test("plays the file to check it", async () => {
    openSettings("needs-a-match", odysseyServer(newAsked()));
    fireEvent.click(await screen.findByRole("link", { name: "The Odyssey (2026)" }));

    const play = await screen.findByRole("link", { name: "Play to check The Odyssey (2026)" });

    expect(play.getAttribute("href")).toBe("/libraries/1/unmatched/70/play");
  });

  test("keeps where the file is behind File details", async () => {
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(newAsked()));
    const details = await screen.findByRole("button", { name: "File details" });
    const path = screen.getByText(odyssey.path);
    expect(details.getAttribute("aria-expanded")).toBe("false");
    expect(path.hidden).toBe(true);

    fireEvent.click(details);

    expect(details.getAttribute("aria-expanded")).toBe("true");
    expect(path.hidden).toBe(false);
  });
});

describe("finding the title", () => {
  test("searches as the Title and Year are typed, with no Search button", async () => {
    const asked = newAsked();
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(asked));
    await screen.findAllByRole("radio", { name: /^The Odyssey/u });
    expect(screen.queryByRole("button", { name: "Search" })).toBeNull();
    expect(asked.searched[0]).toMatchObject({ title: "The Odyssey", year: 2026, libraryId: 1n });

    fireEvent.change(screen.getByRole("searchbox", { name: "Title" }), {
      target: { value: "Odyssey" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Year" }), { target: { value: "" } });

    await waitFor(() => {
      expect(asked.searched.at(-1)?.title).toBe("Odyssey");
    });
    expect(asked.searched.at(-1)?.year).toBeUndefined();
  });

  test("searches once typing pauses, not at each key", async () => {
    const asked = newAsked();
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(asked));
    await screen.findAllByRole("radio", { name: /^The Odyssey/u });
    const searchedBefore = asked.searched.length;
    const title = screen.getByRole("searchbox", { name: "Title" });

    for (const typed of ["O", "Od", "Ody"]) {
      fireEvent.change(title, { target: { value: typed } });
    }

    await waitFor(() => {
      expect(asked.searched.at(-1)?.title).toBe("Ody");
    });
    expect(asked.searched.length - searchedBefore).toBe(1);
  });

  test("searches nothing for a blank Title", async () => {
    const asked = newAsked();
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(asked));
    await screen.findAllByRole("radio", { name: /^The Odyssey/u });
    const searchedBefore = asked.searched.length;

    fireEvent.change(screen.getByRole("searchbox", { name: "Title" }), { target: { value: " " } });

    await expect(screen.findByText("Type a title to search for.")).resolves.toBeDefined();
    expect(asked.searched).toHaveLength(searchedBefore);
  });

  test("lights the first title found, and picking another lights it instead", async () => {
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(newAsked()));
    const first = await screen.findByRole("radio", { name: /^The Odyssey \(2026\)$/u });
    const second = screen.getByRole("radio", { name: /Odyssey$/u });
    expect(first.getAttribute("aria-checked")).toBe("true");
    expect(second.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("group", { name: "Best fits" })).toBeDefined();
    expect(screen.getByText("Odyssey")).toBeDefined();

    fireEvent.click(second);

    expect(first.getAttribute("aria-checked")).toBe("false");
    expect(second.getAttribute("aria-checked")).toBe("true");
  });

  test("shows the first 3 titles and the rest behind N more results", async () => {
    const many = [1, 2, 3, 4, 5].map((n) =>
      found({ value: String(n), title: `Title ${n}`, year: 2000 + n }),
    );
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(newAsked(), many));
    await screen.findByRole("radio", { name: "Title 3 (2003)" });
    expect(screen.queryByRole("radio", { name: "Title 4 (2004)" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "2 more results" }));

    expect(screen.getByRole("radio", { name: "Title 5 (2005)" })).toBeDefined();
    expect(screen.queryByRole("button", { name: /more results/u })).toBeNull();
  });
});

describe("matching", () => {
  test("matches to the title picked with the one Match, then the file is off the list", async () => {
    const asked = newAsked();
    const router = openApp("/code?settings=needs-a-match&item=7", odysseyServer(asked));
    await screen.findAllByRole("radio", { name: /^The Odyssey/u });
    fireEvent.click(screen.getByRole("radio", { name: /Odyssey$/u }));

    fireEvent.click(screen.getByRole("button", { name: "Match" }));

    await expect(screen.findByText("Everything is Matched")).resolves.toBeDefined();
    expect(asked.matched).toHaveLength(1);
    expect(asked.matched[0]).toMatchObject({
      itemId: 7n,
      titleId: { namespace: "tmdb", value: "2" },
    });
    await waitFor(() => {
      expect(router.state.location.search.item).toBeUndefined();
    });
    expect(screen.queryByRole("link", { name: "The Odyssey (2026)" })).toBeNull();
  });

  test("matches to the first title found if none is picked", async () => {
    const asked = newAsked();
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(asked));
    await screen.findAllByRole("radio", { name: /^The Odyssey/u });

    fireEvent.click(screen.getByRole("button", { name: "Match" }));

    await waitFor(() => {
      expect(asked.matched[0]?.titleId).toMatchObject({ value: "1" });
    });
  });

  test("can't Match while nothing is found", async () => {
    openApp("/code?settings=needs-a-match&item=7", odysseyServer(newAsked(), []));

    await expect(
      screen.findByText("Nothing found. Try another title or year."),
    ).resolves.toBeDefined();

    expect(screen.getByRole("button", { name: "Match" }).hasAttribute("disabled")).toBe(true);
  });
});
