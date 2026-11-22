import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import { useQueryClient } from "@tanstack/react-query";

const serversKey = createConnectQueryKey({
  schema: LinkService.method.listLinkedServers,
  cardinality: "finite",
});

/**
 * The signed-in Account's Linked Servers, oldest first. It doesn't suspend: the Hub web app has no
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
