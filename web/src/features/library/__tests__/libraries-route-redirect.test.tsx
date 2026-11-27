import { cleanup, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { openApp } from "@/test-app";
import { fakeServer, newUser } from "@/test-server";

afterEach(cleanup);

test.each(["/libraries", "/libraries/"])(
  "%s gives way to Home, and back skips it",
  async (path) => {
    const router = openApp("/code", fakeServer({ signedIn: newUser }).transport);
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/code");
    });

    router.history.push(path);
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/");
    });

    router.history.back();
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/code");
    });
  },
);

test("signed out, /libraries sends you to sign in first", async () => {
  const router = openApp("/libraries", fakeServer().transport);

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/sign-in");
  });
  expect({ ...router.state.location.search }).toStrictEqual({ redirect: "/libraries" });
});
