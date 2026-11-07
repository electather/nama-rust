import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import type { ReactNode } from "react";

import { DeviceNameField, FormField } from "@/components/form-field";
import { SignInLayout } from "@/components/sign-in-layout";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SessionDelivery, SignInService } from "@/gen/nama/v1/sign_in_pb";
import { type FormErrors, formText, refusal, serverMessage } from "@/lib/forms";

/** Signing this browser in with a User's name and password. */
export function PasswordSignIn({
  onSignedIn,
  footer,
}: {
  /** Called once this browser holds its session. */
  onSignedIn: () => Promise<void>;
  /** Another way to sign in instead. */
  footer: ReactNode;
}) {
  const server = useQuery(ServerService.method.getServerInfo);
  const signIn = useMutation(SignInService.method.signIn, { onSuccess: onSignedIn });
  const errors = signIn.error ? signInErrors(signIn.error) : {};

  return (
    <SignInLayout
      heading={server.data ? `Sign in to ${server.data.name}` : "Sign in"}
      intro="Sign in with the name and password you were given for this Server."
      footer={footer}
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      error={errors.form}
      onSubmit={(form) => {
        signIn.mutate({
          name: formText(form, "name"),
          password: formText(form, "password"),
          deviceName: formText(form, "deviceName"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <FormField label="Name" name="name" autoComplete="username" maxLength={64} />
      <FormField
        label="Password"
        name="password"
        type="password"
        autoComplete="current-password"
        error={errors.password}
      />
      <DeviceNameField error={errors.deviceName} />
    </SignInLayout>
  );
}

function signInErrors(error: unknown): FormErrors {
  return refusal(error, "sign in", {
    // The Server doesn't say whether it was the name or the password, so neither do we.
    [Code.Unauthenticated]: { password: "That name and password don’t match." },
    [Code.InvalidArgument]: { deviceName: serverMessage(error) },
  });
}
