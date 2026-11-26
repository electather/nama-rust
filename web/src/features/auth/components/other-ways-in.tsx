import { Link } from "@tanstack/react-router";

import { PasskeySignIn } from "@/features/auth/components/passkey-sign-in";
import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * Under the Hub's sign-in form: a Sign-in Link instead, also for a forgotten password, on a Hub
 * that `sendsSignInLinks`, a passkey, on one that `offersPasskeys` in a browser that has them,
 * then creating an Account. Each goes on to `redirect`.
 */
export function OtherWaysIn({
  redirect,
  sendsSignInLinks,
  offersPasskeys,
}: {
  redirect: string | undefined;
  sendsSignInLinks: boolean;
  offersPasskeys: boolean;
}) {
  return (
    <>
      {sendsSignInLinks && (
        <SwitchSignInWay
          to="/send-sign-in-link"
          redirect={redirect}
          label="Email me a Sign-in Link"
        />
      )}
      <PasskeySignIn
        redirect={redirect}
        offersPasskeys={offersPasskeys}
        withOr={!sendsSignInLinks}
      />
      {sendsSignInLinks && (
        <p className="text-center text-xs text-pretty text-muted-foreground">
          Forgot your password? Sign in with a Sign-in Link.
        </p>
      )}
      <p className="text-center text-sm text-muted-foreground">
        New to the Hub?{" "}
        <Link to="/create-account" search={{ redirect }} className={textLink}>
          Create an Account
        </Link>
      </p>
    </>
  );
}
