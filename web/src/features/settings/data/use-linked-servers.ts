import { useSuspenseQuery } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** The Linked Servers the signed-in Account owns or has a User on, oldest first. */
export function useLinkedServers() {
  return useSuspenseQuery(LinkService.method.listLinkedServers, {}).data.servers;
}
