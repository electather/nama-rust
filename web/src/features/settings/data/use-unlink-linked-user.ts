import { useMutation } from "@connectrpc/connect-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-identity-change";
import { useRefreshLinks } from "@/features/auth/data/use-linked-users";
import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** Unlinks one of the signed-in Account's Users, which may take their Server off its Servers. */
export function useUnlinkLinkedUser() {
  return useMutation(LinkService.method.unlinkLinkedUser, {
    onError: useSignInIfSessionEnded(),
    onSuccess: useRefreshLinks(),
  });
}
