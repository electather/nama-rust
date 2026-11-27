import { Link } from "@tanstack/react-router";

import { useHubAccount } from "@/shared/hooks/use-hub-account";
import { textLink } from "@/shared/lib/text-link-classes";

/** The way back to a password, left out for a signed-in Account, since `/sign-in` bounces back for one. */
export function PasswordLink({ redirect }: { redirect: string | undefined }) {
  if (useHubAccount() !== null) {
    return null;
  }
  return (
    <p className="text-center text-sm text-muted-foreground">
      <Link to="/sign-in" search={{ redirect }} replace className={textLink}>
        Sign in with a password
      </Link>
    </p>
  );
}
