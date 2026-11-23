import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type GetPersonResponseSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const tmdb = (value: string) => ({ namespace: "tmdb", value });
const weaver = {
  name: "Sigourney Weaver",
  part: "Ellen Ripley",
  personId: tmdb("10205"),
  headshotUrl: "/media/artwork/tmdb/%2Fweaver.jpg",
};
const scott = { name: "Ridley Scott", part: "Director", personId: tmdb("578") };

/** Sigourney Weaver's page: Alien and Aliens in the Movies Library 1 under Cast, nothing under Crew. */
const weaverPage: MessageInitShape<typeof GetPersonResponseSchema> = {
  name: "Sigourney Weaver",
  headshotUrl: weaver.headshotUrl,
  cast: [
    {
      title: {
        case: "movie",
        value: { libraryId: 1n, id: tmdb("679"), title: "Aliens", year: 1986 },
      },
      parts: ["Ripley"],
    },
    {
      title: {
        case: "movie",
        value: { libraryId: 1n, id: tmdb("348"), title: "Alien", year: 1979 },
      },
      parts: ["Ellen Ripley"],
    },
  ],
};

/** A fake Server with a Movies Library (1) holding Alien, whose page and Weaver's it answers. */
function server() {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: tmdb("348"),
          title: "Alien",
          versions: [{ id: 1n }],
          cast: [weaver],
          crew: [scott],
        },
      }),
      getPerson: (request) => {
        if (request.personId?.value !== "10205") {
          throw new ConnectError("no such Person", Code.NotFound);
        }
        return weaverPage;
      },
    });
  });
}

/** Where `link` goes. */
function href(link: HTMLElement) {
  return link.getAttribute("href");
}

test("every name on a title's page opens its Person's page", async () => {
  openApp("/libraries/1/movies/tmdb-348", server());

  const starring = await screen.findByText(/^Starring/u);
  const castAndCrew = within(screen.getByRole("list", { name: "Cast & Crew" }));

  expect(href(within(starring).getByRole("link", { name: "Sigourney Weaver" }))).toBe(
    "/people/tmdb-10205",
  );
  expect(href(castAndCrew.getByRole("link", { name: "Sigourney Weaver Ellen Ripley" }))).toBe(
    "/people/tmdb-10205",
  );
  // Directed by's is the one named only "Ridley Scott": Cast & Crew's says Director too.
  expect(href(screen.getByRole("link", { name: "Ridley Scott" }))).toBe("/people/tmdb-578");
});

test("a Person's page shows their headshot and name over their titles, newest first, with their part", async () => {
  openApp("/people/tmdb-10205", server());

  const heading = await screen.findByRole("heading", { level: 1, name: "Sigourney Weaver" });
  const cast = within(screen.getByRole("list", { name: "Cast" }));

  expect(heading.parentElement?.querySelector("img")?.getAttribute("src")).toBe(weaver.headshotUrl);
  expect(cast.getAllByRole("listitem").map((title) => title.textContent)).toStrictEqual([
    "Aliens1986 · Ripley",
    "Alien1979 · Ellen Ripley",
  ]);
  expect(cast.getByRole("link", { name: /^Alien 1979/u }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-348",
  );
  expect(screen.queryByRole("heading", { name: "Crew" })).toBeNull();
});

test("a Person in no title on the Server is not found", async () => {
  openApp("/people/tmdb-1", server());

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});
