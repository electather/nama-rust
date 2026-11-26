import { type MessageInitShape, create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type Interceptor,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { type HitSchema, type SearchResponseSchema, SearchService } from "@/gen/nama/v1/search_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
});

type Search = ServiceImpl<typeof SearchService>["search"];
type Part = MessageInitShape<typeof SearchResponseSchema>;
type Title = NonNullable<MessageInitShape<typeof HitSchema>["title"]>;

const tmdb = (value: string) => ({ namespace: "tmdb", value });

/** Alien, in the Library `libraryId`. */
const alien = (libraryId: bigint) =>
  ({ case: "movie", value: { libraryId, id: tmdb("348"), title: "Alien", year: 1979 } }) as const;
const aliens = {
  case: "movie",
  value: { libraryId: 1n, id: tmdb("679"), title: "Aliens" },
} as const;
const alienNation = {
  case: "show",
  value: { libraryId: 8n, id: tmdb("4089"), title: "Alien Nation" },
} as const;

/** A search that finds `titles`, in one batch, then says it's done. */
function finds(...titles: Title[]): Search {
  return () =>
    new ReadableStream<Part>({
      start(controller) {
        controller.enqueue({
          part: { case: "hits", value: { hits: titles.map((title) => ({ title })) } },
        });
        controller.enqueue({ part: { case: "done", value: {} } });
        controller.close();
      },
    });
}

/** A search that never answers. */
const hangs: Search = () => new ReadableStream<Part>();

/** A search that fails. */
const fails: Search = () => {
  throw new ConnectError("down", Code.Internal);
};

/** A search that finds Alien Nation for "alien", and never answers for anything else. */
const findsOnlyAlienNation: Search = (request, context) =>
  request.query === "alien" ? finds(alienNation)(request, context) : hangs(request, context);

/**
 * A fake Server that answers once `up` has resolved, signs the Hub's Account in and answers a
 * search with `search`.
 */
function searching(search: Search, interceptors: Interceptor[], up: Promise<void>) {
  return createRouterTransport(
    (router) => {
      router.service(ServerService, {
        getServerInfo: async () => {
          await up;
          return { version: "0.3.0" };
        },
      });
      router.service(SignInService, {
        signInWithHubToken: () => ({ token: "session" }),
        whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
      });
      router.service(SearchService, { search });
    },
    { transport: { interceptors } },
  );
}

const unreachable = () =>
  createRouterTransport(({ service }) => {
    service(ServerService, {
      getServerInfo: () => {
        throw new ConnectError("Failed to fetch", Code.Unknown);
      },
    });
  });

/** The address of the Linked Server `id`. */
const origin = (id: string) => `https://${id}.example`;

/**
 * Opens the Hub's Home for an Account on a Server named for each search in `at`, which answers
 * with it once `up` has resolved, and on Attic, which is off.
 */
function openAtHub(at: Record<string, Search>, up = Promise.resolve()) {
  vi.stubGlobal("fetch", async () => {
    await Promise.resolve();
    throw new TypeError("Failed to fetch");
  });
  const { transport } = accountHub({
    servers: [...Object.keys(at), "attic"].map((id) =>
      create(LinkedServerSchema, { id, name: id, addresses: [origin(id)] }),
    ),
  });
  const searches = new Map(Object.entries(at).map(([id, search]) => [origin(id), search]));
  return openApp("/", transport, undefined, (server, interceptors) => {
    const search = searches.get(server);
    return search === undefined ? unreachable() : searching(search, interceptors, up);
  });
}

/** Opens the action bar from the Hub's Home and types `words` into it. */
async function searchFor(words: string) {
  fireEvent.click(await screen.findByRole("button", { name: "Search your Servers" }));
  const field = await screen.findByRole("combobox", { name: "Command menu" });
  fireEvent.change(field, { target: { value: words } });
}

/** The rows listed, top to bottom, each by all it says. */
function rows() {
  return screen.getAllByRole("option").map((row) => row.textContent);
}

test("search at the Hub lists the titles on every Server, each once", async () => {
  openAtHub({
    basement: finds(alien(1n), aliens),
    cabin: finds(alien(7n), alienNation),
  });

  await searchFor("alien");

  await waitFor(() => {
    expect(rows()).toStrictEqual(["Alien1979Open", "Aliens", "Alien Nation"]);
  });
});

test("a title found at the Hub opens on the Server it was found on", async () => {
  const router = openAtHub({
    basement: finds(aliens),
    cabin: finds(alienNation),
  });
  await searchFor("alien");

  fireEvent.click(await screen.findByRole("option", { name: /^Alien Nation/u }));

  await waitFor(() => {
    expect(router.state.location.href).toBe("/libraries/8/shows/tmdb-4089?server=cabin");
  });
});

test("a slow Server holds up none of the others' results at the Hub", async () => {
  openAtHub({ basement: finds(aliens), cabin: hangs });

  await searchFor("alien");

  await waitFor(() => {
    expect(rows()).toStrictEqual(["AliensOpen"]);
  });
});

test("a Server that fails lists no other Server's answer to the last words at the Hub", async () => {
  openAtHub({ basement: fails, cabin: findsOnlyAlienNation });
  await searchFor("alien");
  await screen.findByRole("option", { name: /^Alien Nation/u });

  fireEvent.change(screen.getByRole("combobox", { name: "Command menu" }), {
    target: { value: "zzz" },
  });

  await expect(screen.findByText("No results for “zzz”")).resolves.toBeInstanceOf(HTMLElement);
  expect(screen.queryByRole("option", { name: /^Alien Nation/u })).toBeNull();
});

test("a search typed before any Server is reached waits for them at the Hub", async () => {
  let reach: (() => void) | undefined;
  openAtHub(
    { basement: finds(aliens) },
    new Promise<void>((resolve) => {
      reach = resolve;
    }),
  );
  await searchFor("alien");
  await new Promise((resolve) => {
    setTimeout(resolve, 400);
  });
  expect(screen.queryByText("No results for “alien”")).toBeNull();

  reach?.();

  await waitFor(() => {
    expect(rows()).toStrictEqual(["AliensOpen"]);
  });
});
