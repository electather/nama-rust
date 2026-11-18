import type { User } from "@/gen/nama/v1/sign_in_pb";
import { DeviceList } from "@/shared/components/device-list/device-list";

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
