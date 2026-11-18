import { useDeleteUser } from "@/features/admin-users/data/use-delete-user";
import { onlyAdminRefusal } from "@/features/admin-users/lib/user-refusals";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";

/** Deletes `user`, once confirmed. */
export function DeleteUser({ user }: { user: User }) {
  const remove = useDeleteUser();
  return (
    <ConfirmDialog
      action="Delete"
      name={`Delete ${user.name}`}
      title={`Delete ${user.name}?`}
      description={`${user.name} can’t sign in any more, and all of their Devices are signed out. This can’t be undone.`}
      onConfirm={async () => {
        await remove.mutateAsync({ userId: user.id });
      }}
      refusal={(error) => onlyAdminRefusal(error, user.name, "delete this User")}
    />
  );
}
