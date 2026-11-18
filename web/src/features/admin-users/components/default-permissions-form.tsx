import { useSetDefaultPermissions } from "@/features/admin-users/data/use-set-default-permissions";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { PermissionsForm } from "@/shared/components/permissions-form/permissions-form";
import { refusal } from "@/shared/lib/forms";
import { permissionsToOffer } from "@/shared/lib/permissions";

/** The form that saves the default Permissions, starting from `initial`. */
export function DefaultPermissionsForm({
  initial,
  you,
}: {
  initial: Permission[];
  you: User | undefined;
}) {
  const save = useSetDefaultPermissions();
  return (
    <PermissionsForm
      initial={initial}
      offer={permissionsToOffer(you)}
      onSave={(permissions) => {
        save.mutate({ permissions });
      }}
      end={{
        action: save.isPending ? "Saving…" : "Save default Permissions",
        pending: save.isPending,
        done: save.isSuccess ? "Default Permissions saved." : undefined,
        refusal: save.error
          ? refusal(save.error, "save the default Permissions", {}).form
          : undefined,
      }}
    />
  );
}
