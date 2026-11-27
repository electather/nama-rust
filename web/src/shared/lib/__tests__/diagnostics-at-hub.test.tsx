import { create } from "@bufbuild/protobuf";
import { type Interceptor, createRouterTransport } from "@connectrpc/connect";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";
import { atOrigin } from "@/test-server-at";

const { recorded } = vi.hoisted(() => {
  const lines: string[] = [];
  return { recorded: lines };
});
// oxlint-disable-next-line vitest/prefer-import-in-mock -- the fake records what each Server's recorder is handed, which the module's types can't say.
vi.mock("@/shared/lib/diagnostics-recorder", () => ({
  recorderAt: (origin: string) => ({
    traceCall:
      (next: Parameters<Interceptor>[0]) => async (request: Parameters<typeof next>[0]) => {
        recorded.push(`${origin} call ${request.method.name}`);
        const response = await next(request);
        return response;
      },
    recordSpan: (name: string, _start: number, attributes: Record<string, string>) => {
      recorded.push(`${origin} span ${name} ${attributes["app.route"]}`);
    },
    recordError: (kind: string) => {
      recorded.push(`${origin} error ${kind}`);
    },
  }),
}));

const basement = "https://basement.example";
const cabin = "https://cabin.example";

afterEach(async () => {
  cleanup();
  // The page's Diagnostics state outlives a test's app, as it would a page's sign-out.
  await recordDiagnosticsIfExported(false, basement);
  await recordDiagnosticsIfExported(false, cabin);
  recorded.length = 0;
  localStorage.clear();
  vi.unstubAllGlobals();
});

/**
 * A fake Server at `origin` that exports Diagnostics or not, signs the Hub's Account in, and
 * answers its Watchlist and Libraries, each empty, recording each Watchlist it lists in `listed`.
 */
function serverAtHub(origin: string, exportsDiagnostics: boolean, listed: string[]) {
  return (interceptors: Interceptor[]) =>
    atOrigin(
      createRouterTransport(
        (router) => {
          router.service(ServerService, {
            getServerInfo: () => ({ version: "0.3.0", exportsDiagnostics }),
          });
          router.service(SignInService, {
            signInWithHubToken: () => ({ token: "session" }),
            whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
          });
          router.service(WatchlistService, {
            listWatchlist: () => {
              listed.push(origin);
              return {};
            },
          });
          router.service(LibraryService, { listLibraries: () => ({}) });
        },
        { transport: { interceptors } },
      ),
      origin,
    );
}

/** Opens `path` at the Hub, for an Account on Basement and Cabin, which export as `exports` says. */
function openAtHub(path: string, exports: Record<string, boolean>, listed: string[] = []) {
  vi.stubGlobal("fetch", async () => {
    await Promise.resolve();
    throw new TypeError("Failed to fetch");
  });
  const { transport } = accountHub({
    servers: ["basement", "cabin"].map((id) =>
      create(LinkedServerSchema, { id, name: id, addresses: [`https://${id}.example`] }),
    ),
  });
  return openApp(path, transport, undefined, (origin, interceptors) =>
    serverAtHub(origin, exports[origin] ?? false, listed)(interceptors),
  );
}

function failUncaught() {
  dispatchEvent(new ErrorEvent("error", { error: new TypeError("not a function") }));
}

test.each([
  [basement, cabin],
  [cabin, basement],
])(
  "at the Hub, a call to a Server is recorded for it only while it exports Diagnostics, whatever the other's setting",
  async (exporter, other) => {
    const listed: string[] = [];
    openAtHub("/watchlist", { [exporter]: true, [other]: false }, listed);

    await vi.waitFor(() => {
      expect(new Set(listed)).toStrictEqual(new Set([basement, cabin]));
    });
    expect(recorded).toContain(`${exporter} call ListWatchlist`);
    expect(recorded.filter((line) => line.startsWith(other))).toStrictEqual([]);
  },
);

test("at the Hub, route changes and uncaught errors on the Hub's own pages are dropped", async () => {
  const listed: string[] = [];
  const router = openAtHub("/watchlist", { [basement]: true }, listed);
  await vi.waitFor(() => {
    expect(listed).toContain(basement);
  });

  await router.navigate({ to: "/" });
  failUncaught();

  expect(recorded.filter((line) => !line.includes(" call "))).toStrictEqual([]);
});

test("at the Hub, route changes and uncaught errors on a Server's page go to that Server", async () => {
  openAtHub("/libraries/1?server=basement", { [basement]: true });
  await vi.waitFor(() => {
    expect(recorded).toContain(
      `${basement} span route change /_member/_server/libraries/$libraryId/`,
    );
  });

  failUncaught();

  expect(recorded).toContain(`${basement} error TypeError`);
});

test("at the Hub, an uncaught error on a Server's page naming no Server, after one that does, is dropped", async () => {
  const router = openAtHub("/libraries/1?server=basement", { [basement]: true });
  await vi.waitFor(() => {
    expect(recorded).toContain(
      `${basement} span route change /_member/_server/libraries/$libraryId/`,
    );
  });
  recorded.length = 0;

  // Pushed, so `?server=` isn't kept: the Hub answers "Page not found".
  router.history.push("/libraries/1");
  await screen.findByRole("heading", { name: "Page not found" });
  failUncaught();

  expect(recorded.filter((line) => !line.includes(" call "))).toStrictEqual([]);
});
