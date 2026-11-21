import { PasswordEnd } from "@/features/settings/components/password-end";
import { PasswordRow } from "@/features/settings/components/password-row";
import { usePasswordChange } from "@/features/settings/hooks/use-password-change";
import { onSubmitted } from "@/shared/lib/forms";

/**
 * The form the signed-in User changes their own password with: their current one, which confirms
 * it's them, and the new one twice. Changing it signs out their other Devices.
 */
export function PasswordForm() {
  const { errors, pending, done, submit } = usePasswordChange();
  return (
    <form
      noValidate
      className="flex flex-col"
      onSubmit={onSubmitted((typed, form) => {
        void submit(typed, form);
      })}
    >
      <PasswordRow
        label="Current password"
        hint="Confirm it’s you."
        name="password"
        autoComplete="current-password"
        error={errors.password}
      />
      <PasswordRow
        label="New password"
        hint="At least 8 characters."
        name="newPassword"
        autoComplete="new-password"
        error={errors.newPassword}
      />
      <PasswordRow
        label="Confirm new password"
        name="confirm"
        autoComplete="new-password"
        error={errors.confirm}
      />
      <PasswordEnd pending={pending} done={done} />
      <p role="alert" className="pt-2.5 text-end text-xs text-destructive empty:hidden">
        {errors.form}
      </p>
    </form>
  );
}
