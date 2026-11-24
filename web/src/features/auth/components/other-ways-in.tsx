import { Link } from "@tanstack/react-router";

import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * Under the Hub's sign-in form: a Sign-in Link instead, also for a forgotten password, which goes
 * on to `redirect`, on a Hub that `sendsSignInLinks`, then creating an Account.
 */
export function OtherWaysIn({
  redirect,
  sendsSignInLinks,
}: {
  redirect: string | undefined;
  sendsSignInLinks: boolean;
}) {
  return (
    <>
      {sendsSignInLinks && (
        <>
          <SwitchSignInWay
            to="/send-sign-in-link"
            redirect={redirect}
            label="Email me a Sign-in Link"
          />
          <p className="text-center text-xs text-pretty text-muted-foreground">
            Forgot your password? Sign in with a Sign-in Link.
          </p>
        </>
      )}
      <p className="text-center text-sm text-muted-foreground">
        New to the Hub?{" "}
        <Link to="/create-account" className={textLink}>
          Create an Account
        </Link>
      </p>
    </>
  );
}
