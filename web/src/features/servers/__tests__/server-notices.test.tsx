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
import { pickFromMenu } from "@/test-menu";

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
 * Opens the Hub's home for an Account on `servers`, each origin's fake Server in `at`, with no User
 * on those in `noUserOn`. Every other origin fails as a `fetch` the browser can't make does: one in
 * `refusingTheHub` answers, but not to the Hub's origin, and the rest can't be reached.
 */
function openHub(
  at: Record<string, ReturnType<typeof fakeServerAt>>,
  servers = [basement, cabin],
  { refusingTheHub = [], noUserOn = [] }: { refusingTheHub?: string[]; noUserOn?: string[] } = {},
) {
  vi.stubGlobal("fetch", async (origin: string) => {
    await Promise.resolve();
    if (!refusingTheHub.includes(origin)) {
      throw new TypeError("Failed to fetch");
    }
    return new Response();
  });
  const { transport } = accountHub({ servers, noUserOn });
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

/** What each notice over Home says, once Cabin's "Films" is in the top bar, so Cabin is reached. */
async function noticesOnceReached() {
  await screen.findByRole("link", { name: "Films" });
  return screen.queryAllByRole("alert").map((notice) => notice.textContent);
}

/** The link in the notice saying `text`. */
async function linkInNotice(text: string) {
  const notice = await screen.findByText(text);
  return within(notice.closest("[role=alert]") ?? document.body).getByRole("link");
}

test("a Server the browser can't reach is noticed as offline, and one Home shows isn't", async () => {
  const served: Served = { trades: [] };
  openHub({ "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) });

  await screen.findByText("Basement is offline.");
  await expect(noticesOnceReached()).resolves.toStrictEqual(["Basement is offline."]);
  expect(served.trades).toStrictEqual(["hub-token-cabin"]);
});

test("a reached Server's Library opens from the bar on that Server", async () => {
  const served: Served = { trades: [] };
  openHub({ "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) }, [
    cabin,
  ]);

  fireEvent.click(await screen.findByRole("link", { name: "Films" }));

  const alien = await screen.findByRole("link", { name: /Alien/u });
  expect(alien.getAttribute("href")).toContain("server=cabin");
});

test("a Server below the minimum version is noticed as needing an update, linking its own web app", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.2.0", served) };
  openHub(at, [cabin]);

  const open = await linkInNotice("Cabin needs an update.");
  expect(open.textContent).toBe("Open Cabin");
  expect(open.getAttribute("href")).toBe("https://cabin.example");
  expect(served.trades).toStrictEqual([]);
});

test("a Server that answers but not to the Hub's origin, as before 0.2.1, needs an update", async () => {
  openHub({}, [cabin], { refusingTheHub: ["https://cabin.example"] });

  await expect(screen.findByText("Cabin needs an update.")).resolves.toBeDefined();
});

test("a Server the Account has no User on is noticed as such", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  openHub(at, [cabin], { noUserOn: ["cabin"] });

  await expect(screen.findByText("Your Account has no User on Cabin.")).resolves.toBeDefined();
  expect(screen.queryByRole("link", { name: "Open Cabin" })).toBeNull();
});

test("on the Hub's HTTPS page, a Server with only a plain-HTTP address links to its own web app", async () => {
  vi.spyOn(globalThis.location, "protocol", "get").mockReturnValue("https:");
  const lan = create(LinkedServerSchema, {
    id: "lan",
    name: "Lan",
    addresses: ["http://192.168.1.20:8096"],
  });
  openHub({}, [lan]);

  const open = await linkInNotice("Lan opens only in its own web app.");
  expect(open.getAttribute("href")).toBe("http://192.168.1.20:8096");
});

test("an Account with no Linked Server is asked to Link one, in Settings, over no rows", async () => {
  openHub({}, []);

  const link = await screen.findByRole("link", { name: "Link a Server" });
  expect(screen.queryAllByRole("alert")).toStrictEqual([]);
  expect(screen.queryAllByRole("region")).toStrictEqual([]);
  fireEvent.click(link);

  await expect(screen.findByRole("form", { name: "Link a Server" })).resolves.toBeDefined();
});

test("another Account signed in later doesn't get the first one's session", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  localStorage.setItem(
    "nama:server-session:v1:someone-else:cabin",
    "https://cabin.example-session",
  );
  openHub(at, [cabin]);

  await expect(screen.findByRole("link", { name: "Films" })).resolves.toBeDefined();
  expect(served.trades).toStrictEqual(["hub-token-cabin"]);
});

test("coming back to the Hub signs in to a Server with the session it kept, not a new Device", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  openHub(at, [cabin]);
  await screen.findByRole("link", { name: "Films" });
  cleanup();

  openHub(at, [cabin]);

  await expect(screen.findByRole("link", { name: "Films" })).resolves.toBeDefined();
  expect(served.trades).toStrictEqual(["hub-token-cabin"]);
});

test("signing the Account out forgets its sessions on its Servers", async () => {
  const served: Served = { trades: [] };
  const at = { "https://cabin.example": fakeServerAt("https://cabin.example", "0.3.0", served) };
  openHub(at, [cabin]);
  await screen.findByRole("link", { name: "Films" });

  await pickFromMenu("ada@example.com’s menu", "Sign out");

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  expect(localStorage.getItem("nama:server-session:v1:0192-ada:cabin")).toBeNull();
});
