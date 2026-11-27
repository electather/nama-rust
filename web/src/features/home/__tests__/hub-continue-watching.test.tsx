import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import type { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { alien, fakeLinkedServer, heat, linksIn, openHub, startedMovie } from "@/test-hub-home";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** Continue Watching's skeleton on Home, if it shows. */
function continueWatchingSkeleton() {
  return screen.getByRole("region", { name: "Continue Watching" }).querySelector(".animate-pulse");
}

test("continue Watching at the Hub shows its skeleton till the first Server answers", async () => {
  let answerBasement: ((answer: ReturnType<typeof listing>) => void) | undefined;
  const basementAnswers = new Promise<ReturnType<typeof listing>>((resolve) => {
    answerBasement = resolve;
  });
  openHub({
    "https://basement.example": fakeLinkedServer({
      listContinueWatching: async () => {
        const answer = await basementAnswers;
        return answer;
      },
    }),
  });
  await waitFor(() => {
    expect(continueWatchingSkeleton()).not.toBeNull();
  });

  answerBasement?.(listing([startedMovie(heat, "Heat", 200n)]));

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "basement" },
    ]);
  });
  expect(continueWatchingSkeleton()).toBeNull();
});

test("a Server whose Continue Watching fails is named, and the others' cards still show", async () => {
  openHub({
    "https://basement.example": fakeLinkedServer({
      listContinueWatching: () => {
        throw new ConnectError("broken", Code.Internal);
      },
    }),
    "https://cabin.example": fakeLinkedServer({
      listContinueWatching: () => listing([startedMovie(heat, "Heat", 200n)]),
    }),
  });

  const region = await screen.findByRole("region", { name: "Continue Watching" });
  await within(region).findByText("Couldn’t load Continue Watching from Basement.");
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

/** `entries` as one answer of Continue Watching, `nextToken` asking for the rest. */
function listing(entries: ReturnType<typeof startedMovie>[], nextToken = "") {
  return { entries, page: { nextToken } };
}

/** The Movies `M0` to `M<count - 1>`, last watched in that order, most recent first. */
function movies(count: number) {
  return Array.from({ length: count }, (_, at) =>
    startedMovie({ namespace: "tmdb", value: `m${at}` }, `M${at}`, BigInt(1000 - at * 10)),
  );
}

/** The titles `M<from>` to `M<to - 1>`. */
function titlesFrom(from: number, to: number) {
  return Array.from({ length: to - from }, (_, at) => `M${from + at}`);
}

/** The names of the cards resuming `titles`. */
function resuming(...titles: string[]) {
  return titles.map((title) => `Resume ${title}, 1h 39m left`);
}

/**
 * The page of `entries` after the one whose token is `token`, 20 to a page, each page's token
 * naming its last title, as the Server's keyset tokens do.
 */
function pageAfter(entries: ReturnType<typeof startedMovie>[], token = "") {
  const from = entries.findIndex(({ movieOrEpisode }) => movieOrEpisode.value.id.value === token);
  const shown = entries.slice(from + 1, from + 21);
  const more = from + 21 < entries.length;
  return listing(shown, more ? shown.at(-1)?.movieOrEpisode.value.id.value : "");
}

/** A fake Server listing `list()` on Continue Watching, a page at a time. */
function pagedServer(
  list: () => ReturnType<typeof startedMovie>[],
  watchState: Partial<ServiceImpl<typeof WatchStateService>> = {},
) {
  return fakeLinkedServer(
    { listContinueWatching: ({ page }) => pageAfter(list(), page?.token) },
    watchState,
  );
}

test("continue Watching at the Hub pages each Server as it scrolls, merged in order, each title once", async () => {
  // Wide enough that the first pages fit across the shelf at once, so it's at its end.
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(4000);
  const basementMovies = movies(25);
  openHub({
    "https://basement.example": pagedServer(() => basementMovies),
    "https://cabin.example": fakeLinkedServer({
      listContinueWatching: () =>
        listing([
          startedMovie({ namespace: "tmdb", value: "m22" }, "M22", 905n),
          startedMovie(heat, "Heat", 805n),
        ]),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching").map(({ name }) => name)).toStrictEqual(
      resuming(
        ...titlesFrom(0, 10),
        "M22",
        ...titlesFrom(10, 20),
        "Heat",
        "M20",
        "M21",
        "M23",
        "M24",
      ),
    );
  });
});

test("a card a change moves onto a Server's next page still shows at the Hub", async () => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(4000);
  let basementMovies = movies(25);
  openHub({
    "https://basement.example": pagedServer(() => basementMovies, {
      setHiddenFromContinueWatching: () => {
        // Meanwhile Alien and Heat are watched elsewhere, which moves M19 onto the next page.
        const others = basementMovies.filter(
          ({ movieOrEpisode }) => movieOrEpisode.value.title !== "M5",
        );
        basementMovies = [
          startedMovie(alien, "Alien", 2000n),
          startedMovie(heat, "Heat", 1900n),
          ...others,
        ];
        return {};
      },
    }),
  });
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toHaveLength(25);
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for M5" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(linksIn("Continue Watching").map(({ name }) => name)).toStrictEqual(
      resuming("Alien", "Heat", ...titlesFrom(0, 5), ...titlesFrom(6, 25)),
    );
  });
});

/**
 * As `pagedServer`, but the page asked for by the token `held` never answers. `asked.held` says
 * whether it's been asked for.
 */
function serverHolding(
  held: string,
  list: () => ReturnType<typeof startedMovie>[],
  watchState: Partial<ServiceImpl<typeof WatchStateService>>,
) {
  const asked = { held: false };
  const server = fakeLinkedServer(
    {
      listContinueWatching: async ({ page }) => {
        if (page?.token === held) {
          asked.held = true;
          await new Promise(() => {});
        }
        return pageAfter(list(), page?.token);
      },
    },
    watchState,
  );
  return { server, asked };
}

test("a Server's later pages still show while a change has them asked for again", async () => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(4000);
  let basementMovies = movies(25);
  // With M5 gone, the first page ends at M20, and the page after it doesn't answer.
  const { server, asked } = serverHolding("m20", () => basementMovies, {
    setHiddenFromContinueWatching: () => {
      basementMovies = basementMovies.filter(
        ({ movieOrEpisode }) => movieOrEpisode.value.title !== "M5",
      );
      return {};
    },
  });
  openHub({ "https://basement.example": server });
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toHaveLength(25);
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for M5" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(asked.held).toBe(true);
  });
  await waitFor(() => {
    expect(linksIn("Continue Watching").map(({ name }) => name)).toStrictEqual(
      resuming(...titlesFrom(0, 5), ...titlesFrom(6, 25)),
    );
  });
});
