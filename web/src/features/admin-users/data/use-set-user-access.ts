import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { usersKey } from "@/shared/hooks/use-users";

/** `SetMaximumAge` and `SetLibraryAccess`, after each of which the Users are fresh. */
export function useSetUserAccess() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const options = {
    // Refused or not, the list may be out of date.
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey(transport) });
    },
  };
  return {
    setMaximumAge: useMutation(UserService.method.setMaximumAge, options),
    setLibraryAccess: useMutation(UserService.method.setLibraryAccess, options),
  };
}
