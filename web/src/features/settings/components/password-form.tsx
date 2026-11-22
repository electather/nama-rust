import { PasswordEnd } from "@/features/settings/components/password-end";
import { PasswordRow } from "@/features/settings/components/password-row";
import { usePasswordChange } from "@/features/settings/hooks/use-password-change";
import { signinLabels } from "@/features/settings/lib/pane-labels";
import { onSubmitted } from "@/shared/lib/forms";
import { rowText } from "@/shared/lib/setting-text";

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
        {...rowText(signinLabels, "password")}
        name="password"
        autoComplete="current-password"
        error={errors.password}
      />
      <PasswordRow
        {...rowText(signinLabels, "newPassword")}
        name="newPassword"
        autoComplete="new-password"
        error={errors.newPassword}
      />
      <PasswordRow
        {...rowText(signinLabels, "confirm")}
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
