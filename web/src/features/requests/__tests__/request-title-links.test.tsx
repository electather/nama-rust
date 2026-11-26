import { create } from "@bufbuild/protobuf";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { RequestService, RequestState } from "@/gen/nama/v1/request_pb";
import { admin, fakeServer, openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const heat = {
  id: 2n,
  movieOrSeason: {
    case: "movie" as const,
    value: { id: { namespace: "tmdb", value: "949" }, title: "Heat" },
  },
  state: RequestState.PENDING,
};

const darkSeason = {
  id: 3n,
  movieOrSeason: {
    case: "season" as const,
    value: { showId: { namespace: "tmdb", value: "70523" }, seasonNumber: 1, showTitle: "Dark" },
  },
  state: RequestState.PENDING,
};

function requestsServer() {
  return fakeServer(
    {
      signedIn: admin,
      serverService: {
        getServerInfo: () => ({ version: "0.3.0", settingsSections: ["requests"] }),
      },
    },
    { signInWithHubToken: () => ({ token: "basement session" }) },
    {},
    (router) => {
      router.service(RequestService, {
        listDefaultQualities: () => ({}),
        listRequests: () => ({ requests: [heat, darkSeason] }),
        countPendingRequests: () => ({}),
        listOfferedQualities: () => ({}),
      });
    },
  );
}

/** Opens `path` at the Hub, for an Account that is an Admin on Basement. */
function openAtHub(path: string) {
  const basement = create(LinkedServerSchema, {
    id: "basement",
    name: "Basement",
    addresses: ["https://basement.example"],
  });
  openApp(path, accountHub({ servers: [basement] }).transport, undefined, requestsServer);
}

async function titleHrefs() {
  const heatLink = await screen.findByRole("link", { name: "Heat" });
  const darkLink = screen.getByRole("link", { name: "Dark" });
  return [heatLink.getAttribute("href"), darkLink.getAttribute("href")];
}

test("at the Hub, a picked Server's Request titles link to that Server's pages", async () => {
  openAtHub("/?settings=requests&manage=basement");

  await expect(titleHrefs()).resolves.toStrictEqual([
    "/movies/tmdb-949?server=basement",
    "/shows/tmdb-70523?server=basement",
  ]);
});

test("at the Hub, a picked Server's Request titles name it over another Server's", async () => {
  openAtHub("/?server=cabin&settings=requests&manage=basement");

  await expect(titleHrefs()).resolves.toStrictEqual([
    "/movies/tmdb-949?server=basement",
    "/shows/tmdb-70523?server=basement",
  ]);
});

test("at a Server, Request titles link to its pages with no Server named", async () => {
  openApp("/?settings=requests", requestsServer());

  await expect(titleHrefs()).resolves.toStrictEqual(["/movies/tmdb-949", "/shows/tmdb-70523"]);
});
