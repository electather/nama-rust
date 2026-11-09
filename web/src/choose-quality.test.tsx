import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { QualitySchema } from "@/gen/nama/v1/plugin_pb";
import {
  type CreateRequestRequest,
  FoundMovieSchema,
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
 * and each ask for it is put in `asked`.
 */
function server(
  permissions: Permission[],
  qualities: MessageInitShape<typeof QualitySchema>[],
  asked: CreateRequestRequest[],
  movie: MessageInitShape<typeof FoundMovieSchema> = dune,
) {
  const found = create(FoundMovieSchema, movie);
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie: found }),
      listMovieQualities: () => ({ qualities }),
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
