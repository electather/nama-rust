import { DeviceList } from "@/components/device-list";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** `user`'s Devices, under a heading. */
export function UserDevices({ user }: { user: User }) {
  return (
    <div>
      <h3 className="text-sm font-medium">Devices</h3>
      <div className="mt-2">
        <DeviceList user={user} />
      </div>
    </div>
  );
}
