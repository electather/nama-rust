import { DeviceItems } from "@/features/settings/components/device-items";
import { OtherDevices } from "@/features/settings/components/other-devices";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { useYourDevices } from "@/features/settings/data/use-your-devices";
import { deviceLabels } from "@/features/settings/lib/pane-labels";

/**
 * Where the signed-in User is signed in: this Device, then the others, most recently used first,
 * each with Revoke, and the ways to revoke them all or sign in another.
 */
export function DevicesPane() {
  const devices = useYourDevices();
  const current = devices.filter((device) => device.isCurrent);
  const others = devices
    .filter((device) => !device.isCurrent)
    // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
    .sort((a, b) => Number(b.lastSeenAt - a.lastSeenAt));
  return (
    <PaneFrame section="devices">
      <SettingGroup setting="current" title={deviceLabels.current.label}>
        <DeviceItems label={deviceLabels.current.label} devices={current} />
      </SettingGroup>
      <OtherDevices others={others} />
    </PaneFrame>
  );
}
