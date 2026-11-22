import { PermissionsFieldset } from "@/features/settings/components/permissions-fieldset";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { DeviceList } from "@/shared/components/device-list/device-list";
import { mayChangePermissions } from "@/shared/lib/permissions";

/** What's under a User's header: their Permissions, as checkboxes when `you` may change them, and their Devices. */
export function UserGroups({
  user,
  you,
  held,
  onChange,
}: {
  user: User;
  you: User | undefined;
  held: Permission[];
  onChange: (changes: { permissions: Permission[] }) => void;
}) {
  return (
    <div className="flex flex-col gap-7.5 px-4 pt-6 pb-4 md:px-7.5 md:pb-7.5">
      {mayChangePermissions(you, user) && (
        <SettingGroup title="Permissions">
          <PermissionsFieldset legend="Permissions" you={you} held={held} onChange={onChange} />
        </SettingGroup>
      )}
      <SettingGroup title="Devices">
        <DeviceList user={user} />
      </SettingGroup>
    </div>
  );
}
