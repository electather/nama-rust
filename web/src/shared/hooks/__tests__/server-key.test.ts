import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";
import { QueryClient } from "@tanstack/react-query";
import { expect, test } from "vite-plus/test";

import { ScanStartedSchema } from "@/gen/nama/v1/event_pb";
import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { scanQuery } from "@/shared/hooks/scan-query";
import { serverQueries } from "@/shared/hooks/server-key";

test("two Servers' data sit in one cache, and each Server's queries are only its own", () => {
  const home = createRouterTransport(() => {});
  const friends = createRouterTransport(() => {});
  const queryClient = new QueryClient();
  const info = (transport: typeof home) =>
    createConnectQueryKey({
      schema: ServerService.method.getServerInfo,
      input: {},
      transport,
      cardinality: "finite",
    });
  const friendsInfo = create(GetServerInfoResponseSchema, { name: "Friend's" });
  queryClient.setQueryData(info(home), create(GetServerInfoResponseSchema, { name: "Home" }));
  queryClient.setQueryData(info(friends), friendsInfo);
  const scanStarted = create(ScanStartedSchema, { libraryId: 1n });
  queryClient.setQueryData(scanQuery(home, 1n).queryKey, scanStarted);
  queryClient.setQueryData(scanQuery(friends, 1n).queryKey, scanStarted);

  const homes = queryClient.getQueryCache().findAll(serverQueries(home));

  expect(homes.map((query) => query.queryKey)).toStrictEqual([
    info(home),
    scanQuery(home, 1n).queryKey,
  ]);
  expect(queryClient.getQueryData(info(friends))).toBe(friendsInfo);
});
