import { useQueryClient } from "@tanstack/react-query";

import { useSensitiveMutation } from "@/components/confirm-its-you";
import { FormEnd, FormField } from "@/components/form-field";
import { UserService } from "@/gen/nama/v1/user_pb";
import { formText, onSubmitted } from "@/lib/forms";
import { changePasswordErrors, devicesKey } from "@/lib/users";

/** The form the signed-in User changes their own password with, which signs out their other Devices. */
export function ChangePassword() {
  const queryClient = useQueryClient();
  const change = useSensitiveMutation(UserService.method.changePassword, {
    // The Server signed out every other Device.
    onSuccess: () => queryClient.invalidateQueries({ queryKey: devicesKey }),
  });
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
