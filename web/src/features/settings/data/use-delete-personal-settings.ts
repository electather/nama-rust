import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { personalKey } from "@/features/settings/data/use-personal-settings";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Forgets a Plugin's Personal settings, then refetches them. */
export function useDeletePersonalSettings() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.deletePersonalSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: personalKey(transport) });
    },
  });
}
