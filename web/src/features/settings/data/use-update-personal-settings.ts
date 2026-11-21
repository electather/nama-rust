import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { personalKey } from "@/features/settings/data/use-personal-settings";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Saves a Plugin's Personal settings, one save at a time, then refetches them. */
export function useUpdatePersonalSettings() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.updatePersonalSettings, {
    // One at a time, in the order they were asked: each save sends every setting, so the last
    // one asked must be the last one the Server takes.
    scope: { id: "personal-settings" },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: personalKey });
    },
  });
}
