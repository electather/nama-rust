import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { notificationPreferencesKey } from "@/features/settings/data/use-notification-preferences";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Turns a kind of notification on or off, then refetches the preferences. */
export function useSetNotificationPreference() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.setNotificationPreference, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: notificationPreferencesKey });
    },
  });
}
