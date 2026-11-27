import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/**
 * Where to look for the link: the same words whether or not an Account has `email`. Signing in
 * with a password instead still goes on to `redirect`.
 */
export function SignInLinkSent({
  email,
  redirect,
}: {
  email: string;
  redirect: string | undefined;
}) {
  return (
    <SignInCard heading="Check your email">
      <p className="text-center text-sm text-pretty">
        If an Account has <span className="break-all">{email}</span>, a Sign-in Link is on its way.
        It works once, within 15 minutes.
      </p>
      <SwitchSignInWay to="/sign-in" redirect={redirect} label="Sign in with a password" />
    </SignInCard>
  );
}
