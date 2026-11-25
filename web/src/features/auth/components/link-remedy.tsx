import { Link } from "@tanstack/react-router";
import { Button, buttonVariants } from "nama-ui/components/button";

import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * What would let a refused code through, if anything: a Sign-in Link, or signing in again, which
 * goes on to `redirect`.
 */
export function LinkRemedy({ error, redirect }: { error: unknown; redirect?: string }) {
  const signOut = useSignOutAccount(redirect);
  const reason = hubRefusalReason(error);
  if (reason === RefusalReason.EMAIL_NOT_PROVEN) {
    return (
      <Link to="/send-sign-in-link" className={buttonVariants({ variant: "ghost" })}>
        Email me a Sign-in Link
      </Link>
    );
  }
  if (
    reason === RefusalReason.SIGN_IN_REQUIRED ||
    reason === RefusalReason.RECENT_AUTHENTICATION_REQUIRED
  ) {
    return (
      <Button
        type="button"
        variant="ghost"
        disabled={signOut.isPending}
        onClick={() => {
          signOut.mutate({});
        }}
      >
        Sign in again
      </Button>
    );
  }
  return null;
}
