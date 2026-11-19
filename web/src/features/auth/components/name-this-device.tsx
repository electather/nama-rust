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
      footer={footer}
      action={pending ? "Getting a code…" : "Get a code"}
      pending={pending}
      errors={errors}
      onSubmit={(form) => {
        onSubmit(formText(form, "deviceName"));
      }}
    >
      <DeviceNameField error={errors.deviceName} />
    </SignInLayout>
  );
}
