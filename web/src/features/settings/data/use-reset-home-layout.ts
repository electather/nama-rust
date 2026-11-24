import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { refetchHomeLayout } from "@/features/settings/data/use-home-layout";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Has the signed-in User's Home follow the default Home layout again, then refetches it. */
export function useResetHomeLayout() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.resetHomeLayout, {
    onSettled: async () => {
      await refetchHomeLayout(queryClient, transport);
    },
  });
}
