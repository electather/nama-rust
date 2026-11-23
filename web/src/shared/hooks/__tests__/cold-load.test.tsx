import type { ConnectRouter } from "@connectrpc/connect";
import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp, type ServerState } from "@/test-app";

afterEach(cleanup);

// oxlint-disable-next-line require-await -- never answers, so whatever waits on it never goes out
const neverAnswers = async () => new Promise<never>(() => {});

const movie = {
  id: { namespace: "tmdb", value: "603" },
  title: "The Matrix",
  versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
};

/** A fake Server in `state` that never lists its Libraries, and the RPCs the app asked it. */
function withLibrariesHeldBack(state: ServerState) {
  const asked: string[] = [];
  const recordedAndHeld = (name: string) => async () => {
    asked.push(name);
    await neverAnswers();
    return {};
  };
  const transport = fakeServer(
    { ...state, libraryService: { listLibraries: neverAnswers } },
    {},
    {},
    (router: ConnectRouter) => {
      router.service(BrowseService, {
        getMovie: () => {
          asked.push("getMovie");
          return { movie };
        },
        getShow: recordedAndHeld("getShow"),
        listEpisodes: recordedAndHeld("listEpisodes"),
        listGenres: recordedAndHeld("listGenres"),
        listUnmatched: recordedAndHeld("listUnmatched"),
      });
      router.service(PlaybackService, { planPlayback: recordedAndHeld("planPlayback") });
    },
  );
  return { transport, asked };
}

test("the session is checked without waiting for the Server's info", async () => {
  let askedWhoAmI = false;
  const whoAmI = async () => {
    askedWhoAmI = true;
    await neverAnswers();
    return {};
  };
  openApp("/", fakeServer({ serverService: { getServerInfo: neverAnswers } }, { whoAmI }));

  await waitFor(() => {
    expect(askedWhoAmI).toBe(true);
  });
});

test.each([
  { path: "/libraries/1", first: ["listGenres", "listUnmatched"] },
  { path: "/libraries/1/movies/tmdb-603", first: ["getMovie"] },
  { path: "/libraries/2/shows/tmdb-1396", first: ["getShow"] },
  { path: "/libraries/1/movies/tmdb-603/play", first: ["planPlayback"] },
  {
    path: "/libraries/2/shows/tmdb-1396/seasons/1/episodes/1/play",
    first: ["listEpisodes"],
  },
])(
  "a cold load of $path asks for its own data before the Libraries answer",
  async ({ path, first }) => {
    const { transport, asked } = withLibrariesHeldBack({ signedIn: newUser });
    openApp(path, transport);

    await waitFor(() => {
      expect(first.every((name) => asked.includes(name))).toBe(true);
    });
  },
);

test("a signed-out visitor is sent to sign in before any page data is asked for", async () => {
  const { transport, asked } = withLibrariesHeldBack({});
  const router = openApp("/libraries/1/movies/tmdb-603/play", transport);

  await expect(screen.findByRole("heading", { name: /^Sign in/u })).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/sign-in");
  expect(asked).toStrictEqual([]);
});
