import { cleanup, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, openApp } from "@/test-app";

afterEach(cleanup);

test("a Movie's page asked for by another of its ids opens its Library by its own id", async () => {
  const server = fakeServer({ signedIn: [Permission.REQUEST_MOVIE] }, {}, {}, (router) => {
    router.service(RequestService, {
      lookUpMovie: () => ({
        movie: { id: { namespace: "tmdb", value: "348" }, title: "Alien", libraryId: 4n },
      }),
    });
  });
  const router = openApp("/movies/imdb-tt0078748", server);

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/4/movies/tmdb-348");
  });
});
