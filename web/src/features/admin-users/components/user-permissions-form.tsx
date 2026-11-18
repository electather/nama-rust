import { useSetPermissions } from "@/features/admin-users/data/use-set-permissions";
import { onlyAdminRefusal } from "@/features/admin-users/lib/user-refusals";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { PermissionsForm } from "@/shared/components/permissions-form/permissions-form";
import { permissionsToOffer } from "@/shared/lib/permissions";

/** The form that saves `user`'s Permissions, offering those `you` may give. */
export function UserPermissionsForm({ user, you }: { user: User; you: User | undefined }) {
  const save = useSetPermissions(user, you);
  return (
    <PermissionsForm
      initial={user.permissions}
      offer={permissionsToOffer(you)}
      onSave={(permissions) => {
        save.mutate({ userId: user.id, permissions });
      }}
      end={{
        action: save.isPending ? "Saving…" : "Save Permissions",
        pending: save.isPending,
        done: save.isSuccess ? "Permissions saved." : undefined,
        refusal: save.error
          ? onlyAdminRefusal(save.error, user.name, "save the Permissions")
          : undefined,
      }}
    />
  );
}
