import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { useSensitiveMutation } from "@/components/confirm-its-you";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { signedInQuery } from "@/lib/access";
import { defaultPermissionsKey, usersKey } from "@/lib/users";

/** `SetPermissions` for `user`, as `you`, after which the Users and your own access are fresh. */
export function useSetPermissions(user: User, you: User | undefined) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const router = useRouter();
  return useSensitiveMutation(UserService.method.setPermissions, {
    onSettled: async () => {
      // Refused or not, the list may be out of date.
      await queryClient.invalidateQueries({ queryKey: usersKey });
      // Your own Permissions decide which pages and links you see, so ask again who is signed in.
      if (user.id === you?.id) {
        // Fetched, not removed: removing would cancel the router's own recheck when the list is
        // refused, and this joins it instead.
        await queryClient.query({ ...signedInQuery(transport), staleTime: 0 });
        await router.invalidate();
      }
    },
  });
}

/** `SetDefaultPermissions`, after which the default Permissions are fresh. */
export function useSetDefaultPermissions() {
  const queryClient = useQueryClient();
  return useSensitiveMutation(UserService.method.setDefaultPermissions, {
    onSettled: () => queryClient.invalidateQueries({ queryKey: defaultPermissionsKey }),
  });
}
