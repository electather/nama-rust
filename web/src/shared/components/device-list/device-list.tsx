import type { User } from "@/gen/nama/v1/sign_in_pb";
import { Devices } from "@/shared/components/device-list/devices";
import { Note } from "@/shared/components/device-list/note";
import { useDevices } from "@/shared/hooks/use-devices";
import { devicesOf } from "@/shared/lib/devices-of";
import { noLongerHere } from "@/shared/lib/no-longer-here";

/** A `user`'s Devices, each but this one with a button to revoke it. Your own without a `user`. */
export function DeviceList({ user }: { user?: User }) {
  const whose = devicesOf(user);
  const { data, error } = useDevices(whose, user !== undefined);
  if (user && error) {
    return <Note>{noLongerHere(user.name).form}</Note>;
  }
  if (!data) {
    return <Note>Loading…</Note>;
  }
  return <Devices devices={data.devices} whose={whose} />;
}
