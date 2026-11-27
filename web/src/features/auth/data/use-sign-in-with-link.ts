import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import {
  AccountService,
  type SignInWithLinkRequestSchema,
  type SignInWithLinkResponse,
} from "@/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * Uses a Sign-in Link up, signing its Account in, then goes where the link said to continue. A link
 * that expired meanwhile has its page look it up again, which then says so.
 */
export function useSignInWithLink() {
  const router = useRouter();
  // A used link can't sign in again, so Back skips its page (`src/routes/AGENTS.md`).
  const signIn = useIdentityChange({
    kind: "account signed in",
    goOn: ({ continueTo }: SignInWithLinkResponse) => continueTo,
  });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SignInWithLinkRequestSchema>) => {
      await signIn(async (hub) => {
        const answer = await callUnaryMethod(hub, AccountService.method.signInWithLink, input);
        return answer;
      });
    },
    onError: async (error) => {
      if (hubRefusalReason(error) === RefusalReason.SIGN_IN_LINK_EXPIRED) {
        await router.invalidate();
      }
    },
  });
}
