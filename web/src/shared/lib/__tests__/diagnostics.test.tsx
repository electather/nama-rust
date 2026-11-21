import { expect, test } from "vite-plus/test";

import { fakeServer, newUser, openApp } from "@/test-app";

/** The `traceparent` the app sends with `ListDevices`, which Settings' Devices pane calls, against a Server that exports Diagnostics or not. */
async function traceparentSent(exportsDiagnostics: boolean) {
  const sent: (string | null)[] = [];
  const transport = fakeServer(
    {
      signedIn: newUser,
      serverService: { getServerInfo: () => ({ name: "Living Room", exportsDiagnostics }) },
    },
    {},
    {
      listDevices: (_request, context) => {
        sent.push(context.requestHeader.get("traceparent"));
        return {};
      },
    },
  );
  openApp("/code?settings=devices", transport);
  await expect.poll(() => sent.length).toBeGreaterThan(0);
  return sent[0];
}

test("a ConnectRPC call sends no traceparent while the Server exports no Diagnostics", async () => {
  await expect(traceparentSent(false)).resolves.toBeNull();
});

test("a ConnectRPC call sends a W3C traceparent while the Server exports Diagnostics", async () => {
  await expect(traceparentSent(true)).resolves.toMatch(/^00-[\da-f]{32}-[\da-f]{16}-01$/u);
});
