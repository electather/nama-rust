import { PermissionsFieldset } from "@/features/settings/components/permissions-fieldset";
import { RemoteCapField } from "@/features/settings/components/remote-cap-field";
import type { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { labelsOf, mayChangeDefaultPermissions } from "@/shared/lib/permissions";

/**
 * The Permissions new Users get, as checkboxes, and their cap for remote playback, when `you` may
 * change them, else the Permissions named.
 */
export function DefaultPermissions({
  you,
  held,
  cap,
  onChange,
}: {
  you: User | undefined;
  held: Permission[];
  cap: PlaybackLimit;
  onChange: (changes: { permissions: Permission[] } | { cap: PlaybackLimit }) => void;
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
    <>
      <PermissionsFieldset legend="Default Permissions" you={you} held={held} onChange={onChange}>
        {about}
      </PermissionsFieldset>
      <RemoteCapField cap={cap} onChange={onChange} />
    </>
  );
}
