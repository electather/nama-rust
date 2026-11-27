import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import type { User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { askAgainWhoIsSignedIn } from "@/shared/hooks/access";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { usersKey } from "@/shared/hooks/use-users";

/** `SetPermissions` for `user`, as `you`, after which the Users and your own access are fresh. */
export function useSetPermissions(user: User, you: User | undefined) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const router = useRouter();
  return useSensitiveMutation(UserService.method.setPermissions, {
    onSettled: async () => {
      // Refused or not, the list may be out of date.
      await queryClient.invalidateQueries({ queryKey: usersKey(transport) });
      // Your own Permissions decide which pages and links you see, so ask again who is signed in.
      if (user.id === you?.id) {
        await askAgainWhoIsSignedIn({ queryClient, transport }, "user", async () => {
          await router.invalidate();
        });
      }
    },
  });
}
