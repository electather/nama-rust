import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { DeviceRow } from "@/shared/components/device-list/device-row";
import { Note } from "@/shared/components/device-list/note";
import type { Whose } from "@/shared/lib/devices-of";

/** The rows of `devices`, or that none is signed in. */
export function Devices({ devices, whose }: { devices: Device[]; whose: Whose }) {
  if (devices.length === 0) {
    return <Note>No Devices are signed in.</Note>;
  }
  return (
    <ul className="flex flex-col">
      {devices.map((device) => (
        <DeviceRow key={device.id} device={device} whose={whose} />
      ))}
    </ul>
  );
}
