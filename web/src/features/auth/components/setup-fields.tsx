import { SetupCodeField } from "@/features/auth/components/setup-code-field";
import { FormField } from "@/shared/components/form-field/form-field";
import { PasswordField } from "@/shared/components/form-field/password-field";
import type { FormErrors } from "@/shared/lib/forms";

/** The fields of the setup form: the Setup Code, then the first Admin. */
export function SetupFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <SetupCodeField error={errors.setupCode} />
      <section aria-labelledby="first-admin" className="flex flex-col gap-2.5 border-t pt-5">
        <h2 id="first-admin" className="text-base font-medium">
          2. Create the first Admin: you
        </h2>
        <FormField
          label="Username"
          name="name"
          variant="lg"
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          maxLength={64}
          error={errors.name}
        />
        <PasswordField
          label="Password"
          variant="lg"
          autoComplete="new-password"
          hint="8 characters or more. There’s no other Admin to reset it, so keep it somewhere safe."
          error={errors.password}
        />
      </section>
    </>
  );
}
