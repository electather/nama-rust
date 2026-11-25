import { useMutation } from "@connectrpc/connect-query";

import { useRefreshLinks } from "@/features/auth/data/use-linked-users";
import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** Accepts a User created from the signed-in Account, by its Invite's id, which Links them to it. */
export function useAcceptUserInvite() {
  return useMutation(LinkService.method.acceptUserInvite, {
    onError: useSignInIfSessionEnded(),
    // Refused or not: an Invite another tab accepted is gone all the same.
    onSettled: useRefreshLinks(),
  });
}
