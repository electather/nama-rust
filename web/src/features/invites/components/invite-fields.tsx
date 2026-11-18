import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";

/** The name and password of the User an Invite creates, with the Server's refusal of each. */
export function InviteFields({ errors }: { errors: FormErrors }) {
  return (
    <>
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
