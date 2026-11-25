import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Linked Servers at the Hub `transport` reaches, for a route's `beforeLoad` to read. */
export function linkedServersQuery(transport: Transport) {
  return {
    ...createQueryOptions(LinkService.method.listLinkedServers, {}, { transport }),
    // Every Server page at the Hub reads it as it loads, so it mustn't ask the Hub on each.
    staleTime: loaderStaleTime,
  };
}

/** The Linked Servers the signed-in Account owns or has a User on, oldest first. */
export function useLinkedServers() {
  return useSuspenseQuery(LinkService.method.listLinkedServers, {}, { staleTime: loaderStaleTime })
    .data.servers;
}
