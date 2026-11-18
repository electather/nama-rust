import { useChangePassword } from "@/features/settings/data/use-change-password";
import { changePasswordErrors } from "@/features/settings/lib/password-refusals";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form the signed-in User changes their own password with, which signs out their other Devices. */
export function ChangePassword() {
  const change = useChangePassword();
  const errors = change.error ? changePasswordErrors(change.error) : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        change.mutate(
          { newPassword: formText(typed, "newPassword") },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <FormField
        label="New password"
        name="newPassword"
        type="password"
        autoComplete="new-password"
        error={errors.newPassword}
      />
      <FormEnd
        action={change.isPending ? "Changing password…" : "Change password"}
        pending={change.isPending}
        done={
          change.isSuccess ? "Password changed. Your other Devices were signed out." : undefined
        }
        refusal={errors.form}
      />
    </form>
  );
}
