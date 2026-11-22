import { useSignOut } from "@hub/features/account/data/sessions";
import type { LinkErrors } from "@hub/features/links/lib/link-refusal";
import { Link } from "@tanstack/react-router";
import { Button, buttonVariants } from "nama-ui/components/button";

/** What would let a refused code through, if anything: signing in again or a Sign-in Link. */
export function LinkRemedy({ errors }: { errors: LinkErrors }) {
  const signOut = useSignOut();
  if (errors.proveEmail === true) {
    return (
      <Link
        to="/send-sign-in-link"
        search={{ continue: "/" }}
        className={buttonVariants({ variant: "ghost" })}
      >
        Email me a Sign-in Link
      </Link>
    );
  }
  return errors.signInAgain === true ? (
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
  ) : undefined;
}
