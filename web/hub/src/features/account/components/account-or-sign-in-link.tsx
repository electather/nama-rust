import { useAccountQuery } from "@hub/features/account/data/account";
import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

/**
 * A way back into the Hub: Your Account when signed in, else Sign in, including when the Hub
 * can't say. Nothing until it answers.
 */
export function AccountOrSignInLink() {
  const { data: account, isPending } = useAccountQuery();
  const className = buttonVariants({ size: "lg" });
  if (isPending) {
    return null;
  }
  return account ? (
    <Link to="/" className={className}>
      Your Account
    </Link>
  ) : (
    <Link to="/sign-in" className={className}>
      Sign in
    </Link>
  );
}
