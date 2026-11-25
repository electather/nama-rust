import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** The query behind `useLinkedServers`, for a route to read the Hub at `transport` with. */
export function linkedServersQuery(transport: Transport) {
  return createQueryOptions(LinkService.method.listLinkedServers, {}, { transport });
}

/** The Linked Servers the signed-in Account owns or has a User on, oldest first. */
export function useLinkedServers() {
  return useSuspenseQuery(LinkService.method.listLinkedServers, {}).data.servers;
}
