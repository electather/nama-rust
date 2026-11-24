import type { ComponentProps } from "react";

import { SignInLinkAccount } from "@/features/auth/components/sign-in-link-account";
import { useSignInWithLink } from "@/features/auth/data/use-sign-in-with-link";
import { hubErrors } from "@/features/auth/lib/hub-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";

/**
 * The page an emailed Sign-in Link, `token`, opens: the Account it signs in, and a button that
 * does. Opening it uses nothing up, since mail scanners open every link (ADR 0100).
 */
export function SignInLink({
  token,
  ...linked
}: { token: string } & ComponentProps<typeof SignInLinkAccount>) {
  const signIn = useSignInWithLink();
  return (
    <SignInLayout
      heading="Sign in"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      errors={signIn.error ? hubErrors(signIn.error) : {}}
      onSubmit={() => {
        signIn.mutate({ token });
      }}
    >
      <SignInLinkAccount {...linked} />
    </SignInLayout>
  );
}
