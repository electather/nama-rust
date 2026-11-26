import { create } from "@bufbuild/protobuf";
import { TransportProvider, createConnectQueryKey } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, Suspense } from "react";

import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { serverTransport } from "@/shared/lib/server";

/**
 * A wrapper rendering inside a transport to the Server at `origin`, whose info, resizing to 320 and
 * 640 wide, the cache already holds, so nothing asks it: a friend's Server, as at the Hub, unless a
 * test names another.
 */
export function onServerAt(origin = "https://friend.example") {
  const transport = serverTransport(origin, []);
  const queryClient = new QueryClient();
  queryClient.setQueryData(
    createConnectQueryKey({
      schema: ServerService.method.getServerInfo,
      input: {},
      transport,
      cardinality: "finite",
    }),
    create(GetServerInfoResponseSchema, { imageWidths: [320, 640] }),
  );
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <TransportProvider transport={transport}>
        <QueryClientProvider client={queryClient}>
          <Suspense>{children}</Suspense>
        </QueryClientProvider>
      </TransportProvider>
    );
  };
}
