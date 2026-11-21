import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { refetchHomeLayout } from "@/features/settings/data/use-home-layout";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Saves the signed-in User's order of their Home rows, then refetches them. */
export function useReorderHomeRows() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.reorderHomeRows, {
    onSettled: async () => {
      await refetchHomeLayout(queryClient);
    },
  });
}
