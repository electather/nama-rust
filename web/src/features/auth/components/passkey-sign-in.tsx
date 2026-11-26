import { OrDivider } from "@/features/auth/components/or-divider";
import { PasskeyButton } from "@/features/auth/components/passkey-button";
import { browserHasPasskeys } from "@/shared/lib/webauthn";

/**
 * Under the Hub's sign-in form: signing in with a passkey, on a Hub that `offersPasskeys` in a
 * browser that has them. `withOr` puts "Or" above it, for when no other way in above drew one.
 */
export function PasskeySignIn({
  redirect,
  offersPasskeys,
  withOr,
}: {
  redirect: string | undefined;
  offersPasskeys: boolean;
  withOr: boolean;
}) {
  if (!offersPasskeys || !browserHasPasskeys()) {
    return null;
  }
  return (
    <>
      {withOr && <OrDivider />}
      <PasskeyButton redirect={redirect} />
    </>
  );
}
