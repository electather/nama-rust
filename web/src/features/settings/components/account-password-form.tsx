import { hubErrors } from "@/features/auth/lib/hub-errors";
import { AccountPasswordEnd } from "@/features/settings/components/account-password-end";
import { HubRefusal } from "@/features/settings/components/hub-refusal";
import { PasswordRow } from "@/features/settings/components/password-row";
import { useHubAccount } from "@/features/settings/data/use-hub-account";
import { useSetAccountPassword } from "@/features/settings/data/use-set-account-password";
import { AlertText } from "@/shared/components/form-field/alert-text";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** What the form says to an Account without a password, and to one with. */
const wording = {
  set: { label: "Password", action: "Set password" },
  change: { label: "New password", action: "Change password" },
};

/**
 * Sets a password on the Account signed in at the Hub, or changes the one it has, which signs it
 * out everywhere else.
 */
export function AccountPasswordForm() {
  const { email, hasPassword } = useHubAccount();
  const setPassword = useSetAccountPassword();
  const words = hasPassword ? wording.change : wording.set;
  const errors = setPassword.error ? hubErrors(setPassword.error) : {};
  return (
    <form
      noValidate
      className="flex flex-col"
      onSubmit={onSubmitted((typed, form) => {
        setPassword.mutate(
          { password: formText(typed, "password") },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <PasswordRow
        setting="password"
        label={words.label}
        hint="8 to 1024 characters."
        name="password"
        autoComplete="new-password"
        error={errors.password}
      />
      {/* Tells a password manager whose password this is. After the row, which draws no hairline
          over itself only as the form's first child. */}
      <input type="email" name="email" autoComplete="username" value={email} readOnly hidden />
      <AccountPasswordEnd
        action={words.action}
        pending={setPassword.isPending}
        done={setPassword.isSuccess}
      />
      <AlertText
        error={
          <HubRefusal
            error={setPassword.error}
            said={errors.form ?? errors.wait}
            toDo="to set your password"
          />
        }
      />
    </form>
  );
}
