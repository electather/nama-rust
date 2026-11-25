import { LibraryAccessField } from "@/features/settings/components/library-access-field";
import { MaximumAgeField } from "@/features/settings/components/maximum-age-field";
import { PermissionsFieldset } from "@/features/settings/components/permissions-fieldset";
import { RemoteCapField } from "@/features/settings/components/remote-cap-field";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { useUserPermissions } from "@/features/settings/hooks/use-user-permissions";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { DeviceList } from "@/shared/components/device-list/device-list";
import { mayChangePermissions } from "@/shared/lib/permissions";

/**
 * What's under a User's header: their Permissions, as checkboxes, their cap for remote playback,
 * Maximum Age and Libraries, when `you` may change them, and their Devices.
 */
export function UserGroups({
  user,
  you,
  values,
  onChange,
}: {
  user: User;
  you: User | undefined;
  values: ReturnType<typeof useUserPermissions>["values"];
  onChange: ReturnType<typeof useUserPermissions>["edit"];
}) {
  const mayChange = mayChangePermissions(you, user);
  return (
    <div className="flex flex-col gap-7.5 px-4 pt-6 pb-4 md:px-7.5 md:pb-7.5">
      {mayChange && (
        <SettingGroup title="Permissions">
          <PermissionsFieldset
            legend="Permissions"
            you={you}
            held={values.permissions}
            onChange={onChange}
          />
          <RemoteCapField cap={values.cap} onChange={onChange} />
        </SettingGroup>
      )}
      {mayChange && (
        <SettingGroup title="What they see">
          <MaximumAgeField maximumAge={values.maximumAge} onChange={onChange} />
          <LibraryAccessField libraryAccess={values.libraryAccess} onChange={onChange} />
        </SettingGroup>
      )}
      <SettingGroup title="Devices">
        <DeviceList user={user} />
      </SettingGroup>
    </div>
  );
}
