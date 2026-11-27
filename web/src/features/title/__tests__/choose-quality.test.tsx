import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { QualitySchema } from "@/gen/nama/v1/plugin_pb";
import {
  type CreateRequestRequest,
  type CreateSeasonRequestsRequest,
  FoundMovieSchema,
  FoundShowSchema,
  RequestSchema,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { fakeServer, newUser } from "@/test-server";

afterEach(cleanup);

const mayChoose = [...newUser, Permission.CHOOSE_QUALITY];

const hd = { id: 1n, profileId: "4", name: "HD", isDefault: true };
const fourK = { id: 2n, profileId: "5", name: "4K", description: "About 50 GB per film" };

const dune = { id: { namespace: "tmdb", value: "438631" }, title: "Dune", year: 2021 };

/**
 * A fake Server for Dune's page, where `dune` is as given, the Movie Fulfiller offers `qualities`,
 * the User's own default is `personalDefault`, and each ask for it is put in `asked`.
 */
// oxlint-disable-next-line max-params -- the tests read as the Server they set up, in this order
function server(
  permissions: Permission[],
  qualities: MessageInitShape<typeof QualitySchema>[],
  asked: CreateRequestRequest[],
  movie: MessageInitShape<typeof FoundMovieSchema> = dune,
  personalDefault?: bigint,
) {
  const found = create(FoundMovieSchema, movie);
  return fakeServer(
    { signedIn: permissions },
    {
      RequestService: {
        lookUpMovie: () => ({ movie: found }),
        listMovieQualities: () => ({ qualities, personalDefault }),
        createRequest: (request) => {
          asked.push(request);
          found.request = create(RequestSchema, {
            state: RequestState.PENDING,
            requesters: [{ userId: 1n, name: "Sam" }],
          });
          return { request: found.request };
        },
      },
    },
  ).transport;
}

/** The chevron beside Request that opens the Qualities to Request `what` at. */
async function openQualities(what: string) {
  const name = `Request ${what} at a Quality`;
  // It's disabled until the Qualities load, and replaced by the menu's own once they do.
  await waitFor(() => {
    expect(screen.getByRole<HTMLButtonElement>("button", { name }).disabled).toBe(false);
  });
  fireEvent.click(screen.getByRole("button", { name }));
  return screen.findAllByRole("menuitemradio");
}

test("request asks for a Movie at once, at the User's Quality", async () => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], asked));

  fireEvent.click(await screen.findByRole("button", { name: "Request Dune" }));

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toStrictEqual([undefined]);
  });
  expect(screen.queryByRole("menu")).toBeNull();
});

test("the Quality menu checks and marks the Fulfiller's default, and each Quality says what it is", async () => {
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], []));

  const items = await openQualities("Dune");

  expect(items.map((item) => [item.textContent, item.getAttribute("aria-checked")])).toStrictEqual([
    ["HD (Default)", "true"],
    ["4KAbout 50 GB per film", "false"],
  ]);
  expect(screen.getByRole("menuitemradio", { description: "About 50 GB per film" })).toBe(items[1]);
});

test("the Quality menu checks the User's own default over the Fulfiller's", async () => {
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], [], dune, 2n));

  const items = await openQualities("Dune");

  expect(items.map((item) => item.getAttribute("aria-checked"))).toStrictEqual(["false", "true"]);
});

test("picking a Quality in the menu Requests the Movie at it", async () => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], asked));

  await openQualities("Dune");
  fireEvent.click(screen.getByRole("menuitemradio", { name: /^4K/u }));

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toStrictEqual([2n]);
  });
});

test("request is disabled while it's being sent", async () => {
  openApp(
    "/movies/tmdb-438631",
    fakeServer(
      { signedIn: newUser },
      {
        RequestService: {
          lookUpMovie: () => ({ movie: dune }),
          createRequest: async () => {
            await new Promise(() => {});
            return {};
          },
        },
      },
    ).transport,
  );
  const request = await screen.findByRole<HTMLButtonElement>("button", { name: "Request Dune" });

  fireEvent.click(request);

  await waitFor(() => {
    expect(request.disabled).toBe(true);
  });
});

test("the Quality menu's chevron holds its place, disabled, while the Qualities load", async () => {
  openApp(
    "/movies/tmdb-438631",
    fakeServer(
      { signedIn: mayChoose },
      {
        RequestService: {
          lookUpMovie: () => ({ movie: dune }),
          listMovieQualities: async () => {
            await new Promise(() => {});
            return {};
          },
        },
      },
    ).transport,
  );

  const chevron = await screen.findByRole<HTMLButtonElement>("button", {
    name: "Request Dune at a Quality",
  });

  expect(chevron.disabled).toBe(true);
});

test.each([
  { who: "may not choose", permissions: newUser, qualities: [hd, fourK] },
  { who: "has one Quality to choose from", permissions: mayChoose, qualities: [hd] },
])("a User who $who gets Request with no Quality menu", async ({ permissions, qualities }) => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(permissions, qualities, asked));

  fireEvent.click(await screen.findByRole("button", { name: "Request Dune" }));

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toStrictEqual([undefined]);
  });
  await waitFor(() => {
    expect(screen.queryByRole("button", { name: "Request Dune at a Quality" })).toBeNull();
  });
});

test("a Movie's page shows its open Request's Quality, and joining it offers no Quality menu", async () => {
  const asked: CreateRequestRequest[] = [];
  const askedByAda = {
    ...dune,
    request: {
      state: RequestState.PENDING,
      requesters: [{ userId: 2n, name: "Ada" }],
      quality: { id: 2n, name: "4K", profileId: "5" },
    },
  };
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], asked, askedByAda));

  await expect(screen.findByText("4K")).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Request Dune" }));

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toStrictEqual([undefined]);
  });
  expect(screen.queryByRole("button", { name: "Request Dune at a Quality" })).toBeNull();
});

const dark = {
  id: { namespace: "tmdb", value: "70523" },
  title: "Dark",
  seasons: [1, 2].map((seasonNumber) => ({ seasonNumber, episodes: [{ episodeNumber: 1 }] })),
};

/**
 * A fake Server for Dark's page, where `show` is as given, the Shows Fulfiller offers `qualities`,
 * and each ask for its Seasons is put in `asked`.
 */
function showServer(
  qualities: MessageInitShape<typeof QualitySchema>[],
  asked: CreateSeasonRequestsRequest[],
  show: MessageInitShape<typeof FoundShowSchema> = dark,
) {
  const found = create(FoundShowSchema, show);
  return fakeServer(
    { signedIn: mayChoose },
    {
      RequestService: {
        lookUpShow: () => ({ show: found }),
        listShowQualities: () => ({ qualities }),
        createSeasonRequests: (request) => {
          asked.push(request);
          return {};
        },
      },
    },
  ).transport;
}

test("request asks for a Show's next Season at once, at the User's Quality", async () => {
  const asked: CreateSeasonRequestsRequest[] = [];
  openApp("/shows/tmdb-70523", showServer([hd, fourK], asked));

  fireEvent.click(await screen.findByRole("button", { name: "Request Season 1 of Dark" }));

  await waitFor(() => {
    expect(asked.map(({ seasonNumbers, quality }) => [seasonNumbers, quality])).toStrictEqual([
      [[1], undefined],
    ]);
  });
});

test("picking a Quality in the menu Requests the Season at it", async () => {
  const asked: CreateSeasonRequestsRequest[] = [];
  openApp("/shows/tmdb-70523", showServer([hd, fourK], asked));

  await screen.findByRole("button", { name: "Request Season 1 of Dark" });
  await openQualities("Season 1 of Dark");
  fireEvent.click(screen.getByRole("menuitemradio", { name: /^4K/u }));

  await waitFor(() => {
    expect(asked.map(({ seasonNumbers, quality }) => [seasonNumbers, quality])).toStrictEqual([
      [[1], 2n],
    ]);
  });
});

test("a Show with a Quality Requests its Seasons with no Quality menu", async () => {
  const asked: CreateSeasonRequestsRequest[] = [];
  const withQuality = { ...dark, quality: { id: 2n, name: "4K", profileId: "5" } };
  openApp("/shows/tmdb-70523", showServer([hd, fourK], asked, withQuality));

  fireEvent.click(await screen.findByRole("button", { name: "Request Season 1 of Dark" }));

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toStrictEqual([undefined]);
  });
  expect(
    screen.queryByRole("button", { name: "Request Season 1 of Dark at a Quality" }),
  ).toBeNull();
});
