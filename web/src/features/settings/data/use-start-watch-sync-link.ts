import { useMutation } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** Starts linking the signed-in User's account at a Watch sync Plugin's tracker; its data is the code to enter. */
export function useStartWatchSyncLink() {
  return useMutation(UserService.method.startWatchSyncLink);
}
