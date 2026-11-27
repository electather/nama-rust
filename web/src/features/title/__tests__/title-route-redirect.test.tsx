import { cleanup, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { fakeServer } from "@/test-server";

afterEach(cleanup);

test("a Movie's page asked for by another of its ids opens its Library by its own id", async () => {
  const { transport } = fakeServer(
    { signedIn: [Permission.REQUEST_MOVIE] },
    {
      RequestService: {
        lookUpMovie: () => ({
          movie: { id: { namespace: "tmdb", value: "348" }, title: "Alien", libraryId: 4n },
        }),
      },
    },
  );
  const router = openApp("/movies/imdb-tt0078748", transport);

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/4/movies/tmdb-348");
  });
});
