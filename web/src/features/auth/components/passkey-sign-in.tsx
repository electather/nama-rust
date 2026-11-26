import { OrDivider } from "@/features/auth/components/or-divider";
import { PasskeyButton } from "@/features/auth/components/passkey-button";
import { browserHasPasskeys } from "@/features/auth/lib/webauthn";

/**
 * Under the Hub's sign-in form: signing in with a passkey, on a Hub that `offersPasskeys` in a
 * browser that has them, after "Or" `withOr`, when it's the first other way in.
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
