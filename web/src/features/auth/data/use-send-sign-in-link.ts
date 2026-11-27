import { useMutation } from "@connectrpc/connect-query";
import { useNavigate } from "@tanstack/react-router";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Asks the Hub to email a Sign-in Link, then says where to look for it, keeping `redirect`. */
export function useSendSignInLink(redirect: string | undefined) {
  const navigate = useNavigate();
  return useMutation(AccountService.method.sendSignInLink, {
    onSuccess: async (_, { email }) => {
      await navigate({
        to: "/sign-in-link-sent",
        search: { redirect },
        state: { signInLinkSentTo: email ?? "" },
        replace: true,
      });
    },
  });
}
