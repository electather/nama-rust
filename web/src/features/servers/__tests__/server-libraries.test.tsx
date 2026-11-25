import { create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type HandlerContext,
  type Interceptor,
  createRouterTransport,
} from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});
const cabin = create(LinkedServerSchema, {
  id: "cabin",
  name: "Cabin",
  url: "https://cabin.example",
  addresses: ["https://cabin.example"],
});

/** What each fake Server at an origin did: the Hub tokens it traded. */
interface Served {
  trades: string[];
}

/**
 * A fake Server at `origin` running `version`, whose one Movies Library, "Films", holds "Alien".
 * It trades the Hub's token for it for the session `<origin>-session`, and answers everything else
 * only with that session.
 */
function fakeServerAt(origin: string, version: string, served: Served) {
  const session = `${origin}-session`;
  const signedInOnly = (context: HandlerContext) => {
    if (context.requestHeader.get("Authorization") !== `Bearer ${session}`) {
      throw new ConnectError("signed out", Code.Unauthenticated);
    }
  };
  return (interceptors: Interceptor[]) =>
    createRouterTransport(
      (router) => {
        router.service(ServerService, { getServerInfo: () => ({ name: origin, version }) });
        router.service(SignInService, {
          signInWithHubToken: ({ hubToken }) => {
            served.trades.push(hubToken);
            return { token: session };
          },
          whoAmI: (_, context) => {
            signedInOnly(context);
            return { user: { id: 1n, name: "Ada" } };
          },
        });
        router.service(LibraryService, {
          listLibraries: (_, context) => {
            signedInOnly(context);
            return { libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] };
          },
        });
        router.service(BrowseService, {
          listUnmatched: () => ({}),
          listGenres: () => ({}),
          listMovies: (_, context) => {
            signedInOnly(context);
            return { movies: [{ id: { namespace: "tmdb", value: "348" }, title: "Alien" }] };
          },
        });
      },
      { transport: { interceptors } },
    );
}

/**
 * Opens the Hub's home for an Account on `servers`, each origin's fake Server in `at`. Every other
 * origin fails as a `fetch` the browser can't make does: one in `refusingTheHub` answers, but not
 * to the Hub's origin, and the rest can't be reached.
 */
function openHub(
  at: Record<string, ReturnType<typeof fakeServerAt>>,
  servers = [basement, cabin],
  refusingTheHub: string[] = [],
) {
  vi.stubGlobal("fetch", async (origin: string) => {
    await Promise.resolve();
    if (!refusingTheHub.includes(origin)) {
      throw new TypeError("Failed to fetch");
    }
    return new Response();
  });
  const { transport } = accountHub({ servers });
  openApp("/", transport, undefined, (origin, interceptors) => {
    const server = at[origin];
    return server ? server(interceptors) : failedFetch;
  });
}

const failedFetch = createRouterTransport(({ service }) => {
  service(ServerService, {
    getServerInfo: () => {
      throw new ConnectError("Failed to fetch", Code.Unknown);
    },
  });
});

/** The part of the Hub's home under `name`'s heading. */
async function serverNamed(name: string) {
  const heading = await screen.findByRole("heading", { name });
  return within(heading.closest("li") ?? document.body);
}

test("each of the Account's Servers lists its Libraries, and one opens that Server's Library", async () => {
  const served: Served = { trades: [] };
  openHub({
    "https://basement.example": fakeServerAt("https://basement.example", "0.3.0", served),
    "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served),
  });

  const cabinItem = await serverNamed("Cabin");
  fireEvent.click(await cabinItem.findByRole("link", { name: "Films" }));

  // The title's page is that Server's too.
  const alien = await screen.findByRole("link", { name: /Alien/u });
  expect(alien.getAttribute("href")).toContain("server=cabin");
  expect(new Set(served.trades)).toStrictEqual(new Set(["hub-token-basement", "hub-token-cabin"]));
});

test("a Server the browser can't reach shows as offline, and the others still list their Libraries", async () => {
  const served: Served = { trades: [] };
  openHub({ "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) });

  const cabinItem = await serverNamed("Cabin");
  await expect(cabinItem.findByRole("link", { name: "Films" })).resolves.toBeDefined();
  const basementItem = await serverNamed("Basement");
  expect(basementItem.getByText("Offline")).toBeDefined();
});

test("a Server below the minimum version links to its own web app, without signing in to it", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.2.0", served) };
  openHub(at, [cabin]);

  const cabinItem = await serverNamed("Cabin");
  const update = await cabinItem.findByRole("link", { name: "Update this Server" });
  expect(update.getAttribute("href")).toBe("https://cabin.example");
  expect(served.trades).toStrictEqual([]);
});

test("a Server that answers but not to the Hub's origin, as before 0.2.1, links to its own web app", async () => {
  openHub({}, [cabin], ["https://cabin.example"]);

  const cabinItem = await serverNamed("Cabin");
  await expect(cabinItem.findByRole("link", { name: "Update this Server" })).resolves.toBeDefined();
});

test("another Account signed in later doesn't get the first one's session", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  localStorage.setItem(
    "nama:server-session:v1:someone-else:cabin",
    "https://cabin.example-session",
  );
  openHub(at, [cabin]);

  const cabinItem = await serverNamed("Cabin");
  await expect(cabinItem.findByRole("link", { name: "Films" })).resolves.toBeDefined();
  expect(served.trades).toStrictEqual(["hub-token-cabin"]);
});

test("coming back to the Hub signs in to a Server with the session it kept, not a new Device", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  openHub(at, [cabin]);
  const first = await serverNamed("Cabin");
  await first.findByRole("link", { name: "Films" });
  cleanup();

  openHub(at, [cabin]);

  const again = await serverNamed("Cabin");
  await expect(again.findByRole("link", { name: "Films" })).resolves.toBeDefined();
  expect(served.trades).toStrictEqual(["hub-token-cabin"]);
});

test("on the Hub's HTTPS page, a Server with only a plain-HTTP address links to its own web app", async () => {
  vi.spyOn(globalThis.location, "protocol", "get").mockReturnValue("https:");
  const lan = create(LinkedServerSchema, {
    id: "lan",
    name: "Lan",
    addresses: ["http://192.168.1.20:8096"],
  });
  openHub({}, [lan]);

  const lanItem = await serverNamed("Lan");
  const open = await lanItem.findByRole("link", { name: "Open this Server" });
  expect(open.getAttribute("href")).toBe("http://192.168.1.20:8096");
});

test("signing the Account out forgets its sessions on its Servers", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  openHub(at, [cabin]);
  const cabinItem = await serverNamed("Cabin");
  await cabinItem.findByRole("link", { name: "Films" });

  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  expect(localStorage.getItem("nama:server-session:v1:0192-ada:cabin")).toBeNull();
});
