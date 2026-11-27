import type { ReactNode } from "react";

import { CurrentPasswordField } from "@/features/auth/components/current-password-field";
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
      footer={footer}
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      errors={errors}
      onSubmit={(form) => {
        signIn.mutate({
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <FormField
        label="Username"
        name="name"
        variant="lg"
        autoComplete="username"
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        maxLength={64}
        // The Server doesn't say which of the two was wrong, so both are marked.
        aria-invalid={errors.password !== undefined}
      />
      <CurrentPasswordField
        error={errors.password}
        remedy="Ask whoever runs this Server to reset it for you."
      />
    </SignInLayout>
  );
}
