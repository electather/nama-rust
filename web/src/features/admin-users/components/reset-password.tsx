import { useResetPassword } from "@/features/admin-users/data/use-reset-password";
import { resetPasswordRefusal } from "@/features/admin-users/lib/user-refusals";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText } from "@/shared/lib/forms";

/** Sets `user`'s password for an Admin to pass on, once confirmed. */
export function ResetPassword({ user }: { user: User }) {
  const reset = useResetPassword();
  return (
    <ConfirmDialog
      action="Reset password"
      name={`Reset password for ${user.name}`}
      title={`Reset ${user.name}’s password?`}
      description={`All of ${user.name}’s Devices are signed out. Pass the new password on to them.`}
      onConfirm={async (typed) => {
        await reset.mutateAsync({ userId: user.id, newPassword: formText(typed, "newPassword") });
      }}
      refusal={(error) => resetPasswordRefusal(error, user.name)}
    >
      <FormField
        label="New password"
        name="newPassword"
        type="password"
        autoComplete="new-password"
      />
    </ConfirmDialog>
  );
}
