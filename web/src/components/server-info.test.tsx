import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";

import { ServerInfo } from "./server-info";

test("shows the Server's name and version", async () => {
  const transport = createRouterTransport(({ service }) => {
    service(ServerService, {
      getServerInfo: () =>
        create(GetServerInfoResponseSchema, { name: "Living Room", version: "1.2.3" }),
    });
  });

  render(
    <TransportProvider transport={transport}>
      <QueryClientProvider client={new QueryClient()}>
        <ServerInfo />
      </QueryClientProvider>
    </TransportProvider>,
  );

  expect(await screen.findByRole("heading", { name: "Living Room" })).toBeDefined();
  expect(screen.getByText("Nama 1.2.3")).toBeDefined();
});
