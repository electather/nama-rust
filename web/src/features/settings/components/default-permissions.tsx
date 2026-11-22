import { PermissionsFieldset } from "@/features/settings/components/permissions-fieldset";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { labelsOf, mayChangeDefaultPermissions } from "@/shared/lib/permissions";

/** The Permissions new Users get, as checkboxes when `you` may change them, else named. */
export function DefaultPermissions({
  you,
  held,
  onChange,
}: {
  you: User | undefined;
  held: Permission[];
  onChange: (changes: { permissions: Permission[] }) => void;
}) {
  const about = (
    <p className="mt-1 text-sm text-pretty text-muted-foreground">
      What new Users get. Changing them leaves the Users already here as they are.
    </p>
  );
  if (!mayChangeDefaultPermissions(you, held)) {
    return (
      <>
        {about}
        <p className="mt-3 text-sm">{labelsOf(held)}</p>
      </>
    );
  }
  return (
    <PermissionsFieldset legend="Default Permissions" you={you} held={held} onChange={onChange}>
      {about}
    </PermissionsFieldset>
  );
}
