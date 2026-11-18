import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";

/** The fields of the setup form. */
export function SetupFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField
        label="Setup Code"
        name="setupCode"
        autoComplete="off"
        spellCheck={false}
        error={errors.setupCode}
      />
      <FormField
        label="Username"
        name="name"
        autoComplete="username"
        maxLength={64}
        error={errors.name}
      />
      <FormField
        label="Password"
        name="password"
        type="password"
        autoComplete="new-password"
        error={errors.password}
      />
    </>
  );
}
