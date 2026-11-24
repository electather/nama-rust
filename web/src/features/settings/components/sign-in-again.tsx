import { useLocation } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { signedInTooLongAgo } from "@/features/settings/lib/account-refusals";

/**
 * That the Hub wants a sign-in in the last few minutes `toDo` something, and the button that signs
 * in again: it signs out, since the sign-in page sends a signed-in Account on, and comes back here.
 */
export function SignInAgain({ error, toDo }: { error: unknown; toDo: string }) {
  const here = useLocation({ select: (location) => location.href });
  const signOut = useSignOutAccount(here);
  return (
    <>
      {signedInTooLongAgo(error)}{" "}
      <Button
        variant="link"
        size="link"
        disabled={signOut.isPending}
        onClick={() => {
          signOut.mutate({});
        }}
      >
        Sign in again
      </Button>{" "}
      {toDo}.
    </>
  );
}
