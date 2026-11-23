import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import { useQueryClient } from "@tanstack/react-query";

/** The query key of the Linked Servers the signed-in Account owns or has a User on. */
export const serversKey = createConnectQueryKey({
  schema: LinkService.method.listLinkedServers,
  cardinality: "finite",
});

/**
 * The Linked Servers the signed-in Account owns or has a User on, oldest first. It doesn't suspend: the Hub web app has no
 * boundary to catch a failure, and the rest of the Account page works without the list.
 */
export function useLinkedServers() {
  return useQuery(LinkService.method.listLinkedServers, {});
}

/** Links the Server showing a code to the signed-in Account, then lists the Servers again. */
export function useApproveServerLink() {
  const queryClient = useQueryClient();
  return useMutation(LinkService.method.approveServerLink, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: serversKey });
    },
  });
}
