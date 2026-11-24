import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { refetchHomeLayout } from "@/features/settings/data/use-home-layout";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Saves which Home rows, and whether Featured, the signed-in User shows, then refetches them. */
export function useSetHomeLayout() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.setHomeLayout, {
    onSettled: async () => {
      await refetchHomeLayout(queryClient, transport);
    },
  });
}
