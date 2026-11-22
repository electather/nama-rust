import { DeviceItem } from "@/features/settings/components/device-item";
import type { Device } from "@/gen/nama/v1/sign_in_pb";

/** The `devices` of one group of the Devices pane, as a list named `label`. */
export function DeviceItems({ label, devices }: { label: string; devices: Device[] }) {
  return (
    <ul aria-label={label} className="-mb-1.5 flex flex-col empty:hidden">
      {devices.map((device) => (
        <DeviceItem key={device.id} device={device} />
      ))}
    </ul>
  );
}
