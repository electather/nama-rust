import type { ReactNode } from "react";

import { useSignIn } from "@/features/auth/data/use-sign-in";
import { signInErrors } from "@/features/auth/lib/sign-in-errors";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { useServerName } from "@/shared/hooks/use-server-name";
import { formText } from "@/shared/lib/forms";

/** Signing this browser in with a username and password. */
export function PasswordSignIn({
  onSignedIn,
  footer,
}: {
  /** Called once this browser holds its session. */
  onSignedIn: () => Promise<void>;
  /** Another way to sign in instead. */
  footer: ReactNode;
}) {
  const serverName = useServerName();
  const signIn = useSignIn(onSignedIn);
  const errors = signIn.error ? signInErrors(signIn.error) : {};

  return (
    <SignInLayout
      heading={serverName === undefined ? "Sign in" : `Sign in to ${serverName}`}
      intro="Sign in with the username and password you were given for this Server."
      footer={footer}
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      error={errors.form}
      onSubmit={(form) => {
        signIn.mutate({
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <FormField label="Username" name="name" autoComplete="username" maxLength={64} />
      <FormField
        label="Password"
        name="password"
        type="password"
        autoComplete="current-password"
        error={errors.password}
      />
    </SignInLayout>
  );
}
