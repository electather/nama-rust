import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { refetchHomeLayout } from "@/features/settings/data/use-home-layout";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Saves the signed-in User's order of their Home rows, then refetches them. */
export function useReorderHomeRows() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.reorderHomeRows, {
    // One at a time, so each move's whole order reaches the Server after the one before it.
    scope: { id: "reorder-home-rows" },
    onSettled: async () => {
      await refetchHomeLayout(queryClient, transport);
    },
  });
}
