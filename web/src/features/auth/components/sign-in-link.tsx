import type { ComponentProps } from "react";

import { LinkAccount } from "@/features/auth/components/link-account";
import { useSignInWithLink } from "@/features/auth/data/use-sign-in-with-link";
import { accountErrors } from "@/features/auth/lib/account-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";

/**
 * The page an emailed Sign-in Link, `token`, opens: the Account it signs in, and a button that
 * does. Opening it uses nothing up, since mail scanners open every link (ADR 0100).
 */
export function SignInLink({
  token,
  ...linked
}: { token: string } & ComponentProps<typeof LinkAccount>) {
  const use = useSignInWithLink();
  return (
    <SignInLayout
      heading="Sign in"
      action={use.isPending ? "Signing in…" : "Sign in"}
      pending={use.isPending}
      errors={use.error ? accountErrors(use.error) : {}}
      onSubmit={() => {
        use.mutate({ token });
      }}
    >
      <LinkAccount {...linked} />
    </SignInLayout>
  );
}
