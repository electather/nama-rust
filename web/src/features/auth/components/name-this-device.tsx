import type { ReactNode } from "react";

import { DeviceNameField } from "@/features/auth/components/device-name-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { type FormErrors, formText } from "@/shared/lib/forms";

/** The form that names this Device and asks for a Sign-in Code. */
export function NameThisDevice({
  onSubmit,
  pending,
  errors,
  footer,
}: {
  onSubmit: (deviceName: string) => void;
  pending: boolean;
  errors: FormErrors;
  footer: ReactNode;
}) {
  return (
    <SignInLayout
      heading="Sign in with a code"
      intro="Name this Device and get a Sign-in Code. Then enter it on a Device where you’re already signed in."
      footer={footer}
      action={pending ? "Getting a code…" : "Get a code"}
      pending={pending}
      error={errors.form}
      onSubmit={(form) => {
        onSubmit(formText(form, "deviceName"));
      }}
    >
      <DeviceNameField error={errors.deviceName} />
    </SignInLayout>
  );
}
