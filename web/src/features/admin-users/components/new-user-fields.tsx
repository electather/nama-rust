import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";

/** A new User's name and starting password, with any refusal under each. */
export function NewUserFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField
        label="Username"
        name="name"
        autoComplete="off"
        maxLength={64}
        error={errors.name}
      />
      <FormField
        label="Starting password"
        name="password"
        type="password"
        autoComplete="new-password"
        hint="Pass it on to them. They can change it in Settings."
        error={errors.password}
      />
    </>
  );
}
