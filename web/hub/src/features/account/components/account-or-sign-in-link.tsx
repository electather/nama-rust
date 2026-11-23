import { useAccount } from "@hub/features/account/data/account";
import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

/** A way back into the Hub: Your Account when signed in, else Sign in. Suspends until it knows. */
export function AccountOrSignInLink() {
  const className = buttonVariants({ size: "lg" });
  return useAccount() ? (
    <Link to="/" className={className}>
      Your Account
    </Link>
  ) : (
    <Link to="/sign-in" className={className}>
      Sign in
    </Link>
  );
}
