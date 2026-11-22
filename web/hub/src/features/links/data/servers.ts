import { createConnectQueryKey, useMutation, useSuspenseQuery } from "@connectrpc/connect-query";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import { useQueryClient } from "@tanstack/react-query";

const serversKey = createConnectQueryKey({
  schema: LinkService.method.listLinkedServers,
  cardinality: "finite",
});

/** The signed-in Account's Linked Servers, oldest first. */
export function useLinkedServers() {
  return useSuspenseQuery(LinkService.method.listLinkedServers, {}).data.servers;
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
