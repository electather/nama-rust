import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type Interceptor, createRouterTransport } from "@connectrpc/connect";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});
const cabin = create(LinkedServerSchema, {
  id: "cabin",
  name: "Cabin",
  addresses: ["https://cabin.example"],
});

const cabinOffline = "https://cabin-offline.example";

/**
 * A fake Server that signs the Hub's Account in as Ada and has one Library, "Series", except at
 * Cabin's address, which can't be reached.
 */
function fakeServerAt(origin: string, interceptors: Interceptor[]) {
  return createRouterTransport(
    (router) => {
      router.service(ServerService, {
        getServerInfo: () => {
          if (origin === cabinOffline) {
            throw new ConnectError("offline", Code.Unavailable);
          }
          return { version: "0.3.0" };
        },
      });
      router.service(SignInService, {
        signInWithHubToken: () => ({ token: "session" }),
        whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
      });
      router.service(LibraryService, {
        listLibraries: () => ({ libraries: [{ id: 2n, name: "Series", kind: LibraryKind.SHOWS }] }),
      });
    },
    { transport: { interceptors } },
  );
}

function openHub(path: string, servers = [basement]) {
  const { transport } = accountHub({ servers });
  return openApp(path, transport, undefined, fakeServerAt);
}

test("a Library's URL without a Server opens on the Account's only one", async () => {
  const router = openHub("/libraries/2");

  await vi.waitFor(() => {
    expect(router.state.location.href).toBe("/libraries/2?server=basement");
  });
});

test("a Library's URL without a Server opens on the only one of two the Account reaches", async () => {
  const offline = create(LinkedServerSchema, { ...cabin, addresses: [cabinOffline] });
  const router = openHub("/libraries/2", [basement, offline]);

  await vi.waitFor(() => {
    expect(router.state.location.href).toBe("/libraries/2?server=basement");
  });
});

test.each([
  ["two Servers", [basement, cabin]],
  ["no Server", []],
])("a Library's URL without a Server opens Home for an Account with %s", async (_, servers) => {
  const router = openHub("/libraries/2", servers);

  await vi.waitFor(() => {
    expect(router.state.location.href).toBe("/");
  });
});

test("a Library's URL naming a Server the Account hasn't is not found", async () => {
  openHub("/libraries/2?server=attic");

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});
