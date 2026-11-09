import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { QualitySchema } from "@/gen/nama/v1/plugin_pb";
import {
  type CreateFollowedShowRequest,
  type CreateRequestRequest,
  type CreateSeasonRequestsRequest,
  FoundMovieSchema,
  FoundShowSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const mayChoose = [...newUser, Permission.CHOOSE_QUALITY];

const hd = { id: 1n, profileId: "4", name: "HD", isDefault: true };
const fourK = { id: 2n, profileId: "5", name: "4K", description: "About 50 GB per film" };

const dune = { tmdbId: 438_631n, title: "Dune", year: 2021 };

/**
 * A fake Server for Dune's page, where `dune` is as given, the Movie Fulfiller offers `qualities`,
 * the User's own default is `personalDefault`, and each ask for it is put in `asked`.
 */
function server(
  permissions: Permission[],
  qualities: MessageInitShape<typeof QualitySchema>[],
  asked: CreateRequestRequest[],
  movie: MessageInitShape<typeof FoundMovieSchema> = dune,
  personalDefault?: bigint,
) {
  const found = create(FoundMovieSchema, movie);
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie: found }),
      listMovieQualities: () => ({ qualities, personalDefault }),
      createRequest: (request) => {
        asked.push(request);
        found.request = create(RequestSchema, { state: RequestState.PENDING });
        return { request: found.request };
      },
    });
  });
}

async function clickRequest() {
  const button = await screen.findByRole<HTMLButtonElement>("button", { name: "Request Dune" });
  await waitFor(() => {
    expect(button.disabled).toBe(false);
  });
  fireEvent.click(button);
}

test("a User who may choose picks a Quality, the default picked and marked, each with its description", async () => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], asked));

  await clickRequest();
  const dialog = await screen.findByRole("alertdialog", { name: "Request Dune" });
  const picked = within(dialog).getByRole<HTMLInputElement>("radio", { name: "HD (Default)" });
  const other = within(dialog).getByRole<HTMLInputElement>("radio", { name: "4K" });

  expect(picked.checked).toBe(true);
  expect(other.getAttribute("aria-describedby")).not.toBeNull();
  expect(within(dialog).getByText("About 50 GB per film")).toBeDefined();
  fireEvent.click(other);
  fireEvent.click(within(dialog).getByRole("button", { name: "Request" }));
  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toEqual([2n]);
  });
});

test("the picker starts at the User's own default, the Movie Fulfiller's still marked", async () => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(mayChoose, [hd, fourK], asked, dune, 2n));

  await clickRequest();
  const dialog = await screen.findByRole("alertdialog", { name: "Request Dune" });

  expect(within(dialog).getByRole<HTMLInputElement>("radio", { name: "4K" }).checked).toBe(true);
  expect(
    within(dialog).getByRole<HTMLInputElement>("radio", { name: "HD (Default)" }).checked,
  ).toBe(false);
});

test.each([
  { who: "may not choose", permissions: newUser, qualities: [hd, fourK] },
  { who: "has one Quality to choose from", permissions: mayChoose, qualities: [hd] },
])("a User who $who Requests without a picker", async ({ permissions, qualities }) => {
  const asked: CreateRequestRequest[] = [];
  openApp("/movies/tmdb-438631", server(permissions, qualities, asked));

  await clickRequest();

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toEqual([undefined]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("a Movie's page shows its open Request's Quality, and joining it shows no picker", async () => {
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

  expect(await screen.findByText("4K")).toBeDefined();
  await clickRequest();

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toEqual([undefined]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

const dark = {
  tmdbId: 70_523n,
  title: "Dark",
  seasons: [{ seasonNumber: 1 }, { seasonNumber: 2 }],
};

/**
 * A fake Server for Dark's page, where `show` is as given, the Shows Fulfiller offers `qualities`,
 * each ask for its Seasons is put in `asked`, and each ask for the Whole Show in `followed`.
 */
function showServer(
  qualities: MessageInitShape<typeof QualitySchema>[],
  asked: CreateSeasonRequestsRequest[],
  show: MessageInitShape<typeof FoundShowSchema> = dark,
  followed: CreateFollowedShowRequest[] = [],
) {
  const found = create(FoundShowSchema, show);
  return fakeServer({ signedIn: mayChoose }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpShow: () => ({ show: found }),
      listShowQualities: () => ({ qualities }),
      createSeasonRequests: (request) => {
        asked.push(request);
        return {};
      },
      createFollowedShow: (request) => {
        followed.push(request);
        return {};
      },
    });
  });
}

/** Picks Season 2 of Dark and Requests it, once the Qualities are read. */
async function requestSeason2() {
  await requestPicking("Season 2");
}

/** Picks the Whole Show of Dark and Requests it, once the Qualities are read. */
async function requestWholeShow() {
  await requestPicking("Whole Show");
}

/** Picks the choice `name` on Dark's page and Requests it, once the Qualities are read. */
async function requestPicking(name: string) {
  fireEvent.click(await screen.findByRole("checkbox", { name }));
  const button = screen.getByRole<HTMLButtonElement>("button", { name: "Request Seasons of Dark" });
  await waitFor(() => {
    expect(button.disabled).toBe(false);
  });
  fireEvent.click(button);
}

test("a User who may choose picks the Quality of a Show's first Seasons", async () => {
  const asked: CreateSeasonRequestsRequest[] = [];
  openApp("/shows/tmdb-70523", showServer([hd, fourK], asked));

  await requestSeason2();
  const dialog = await screen.findByRole("alertdialog", { name: "Request Seasons of Dark" });
  fireEvent.click(within(dialog).getByRole("radio", { name: "4K" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "Request" }));

  await waitFor(() => {
    expect(asked.map(({ seasonNumbers, quality }) => [seasonNumbers, quality])).toEqual([
      [[2], 2n],
    ]);
  });
});

test("a Show with a Quality shows it, and its later Seasons are Requested without a picker", async () => {
  const asked: CreateSeasonRequestsRequest[] = [];
  const withQuality = { ...dark, quality: { id: 2n, name: "4K", profileId: "5" } };
  openApp("/shows/tmdb-70523", showServer([hd, fourK], asked, withQuality));

  expect(await screen.findByText(/downloads at 4K/u)).toBeDefined();
  await requestSeason2();

  await waitFor(() => {
    expect(asked.map(({ quality }) => quality)).toEqual([undefined]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("a Show whose Quality Sonarr kept and no Quality offers says it is set in the service", async () => {
  const kept = { ...dark, quality: { id: 0n, name: "HD-1080p", profileId: "4" } };
  openApp("/shows/tmdb-70523", showServer([hd, fourK], [], kept));

  expect(await screen.findByText(/downloads at HD-1080p \(set in the service\)/u)).toBeDefined();
});

test("a User who may choose picks the Quality of a Whole Show they follow", async () => {
  const followed: CreateFollowedShowRequest[] = [];
  openApp("/shows/tmdb-70523", showServer([hd, fourK], [], dark, followed));

  await requestWholeShow();
  const dialog = await screen.findByRole("alertdialog", { name: "Request the Whole Show of Dark" });
  fireEvent.click(within(dialog).getByRole("radio", { name: "4K" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "Request" }));

  await waitFor(() => {
    expect(followed.map(({ quality }) => quality)).toEqual([2n]);
  });
});

test("a Show whose Quality is fixed shows it, and its Whole Show is followed without a picker", async () => {
  const followed: CreateFollowedShowRequest[] = [];
  const withQuality = { ...dark, quality: { id: 2n, name: "4K", profileId: "5" } };
  openApp("/shows/tmdb-70523", showServer([hd, fourK], [], withQuality, followed));

  expect(await screen.findByText(/downloads at 4K/u)).toBeDefined();
  await requestWholeShow();

  await waitFor(() => {
    expect(followed.map(({ quality }) => quality)).toEqual([undefined]);
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});
