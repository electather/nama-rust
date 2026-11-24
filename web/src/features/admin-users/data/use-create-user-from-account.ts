import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { usersKey } from "@/shared/hooks/use-users";

/** Creates a User from an Account, once the Admin confirms it's them, then refetches the Users. */
export function useCreateUserFromAccount() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(UserService.method.createUserFromAccount, {
    confirming: "create a User from an Account",
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey(transport) });
    },
  });
}
