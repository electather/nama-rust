import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { FormField } from "@/components/form-field";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { formText } from "@/lib/forms";
import { devicesKey, onlyAdminRefusal, resetPasswordRefusal, usersKey } from "@/lib/users";

/** Sets `user`'s password for an Admin to pass on, once confirmed. */
export function ResetPassword({ user }: { user: User }) {
  const queryClient = useQueryClient();
  const reset = useMutation(UserService.method.resetPassword, {
    // Their Devices were signed out, or they were deleted meanwhile.
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: devicesKey }),
        queryClient.invalidateQueries({ queryKey: usersKey }),
      ]),
  });
  return (
    <ConfirmDialog
      action="Reset password"
      name={`Reset password for ${user.name}`}
      title={`Reset ${user.name}’s password?`}
      description={`All of ${user.name}’s Devices are signed out. Pass the new password on to them.`}
      onConfirm={(typed) =>
        reset.mutateAsync({ userId: user.id, newPassword: formText(typed, "newPassword") })
      }
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

/** Deletes `user`, once confirmed. */
export function DeleteUser({ user }: { user: User }) {
  const queryClient = useQueryClient();
  const remove = useMutation(UserService.method.deleteUser, {
    onSettled: () => queryClient.invalidateQueries({ queryKey: usersKey }),
  });
  return (
    <ConfirmDialog
      action="Delete"
      name={`Delete ${user.name}`}
      title={`Delete ${user.name}?`}
      description={`${user.name} can’t sign in any more, and all of their Devices are signed out. This can’t be undone.`}
      onConfirm={() => remove.mutateAsync({ userId: user.id })}
      refusal={(error) => onlyAdminRefusal(error, user.name, "delete this User")}
    />
  );
}
