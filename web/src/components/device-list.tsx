import { Code } from "@connectrpc/connect";
import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import type { Device, User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { dayOf } from "@/lib/browse";
import { isCode } from "@/lib/connect-errors";
import { refusal } from "@/lib/forms";
import { devicesKey, noLongerHere } from "@/lib/users";

/** A `user`'s Devices, each but this one with a button to revoke it. Your own without a `user`. */
export function DeviceList({ user }: { user?: User }) {
  const whose = devicesOf(user);
  // A failure shows the page's error, as for any other query the page needs, except that another
  // Admin may have deleted the User meanwhile.
  const { data, error } = useQuery(UserService.method.listDevices, whose, {
    throwOnError: (failure) => !(user && isCode(failure, Code.NotFound)),
  });
  if (user && error) return <Note>{noLongerHere(user.name).form}</Note>;
  if (!data) return <Note>Loading…</Note>;
  return <Devices devices={data.devices} whose={whose} />;
}

/** The query for your own Devices, for a page's loader to fetch before it shows. */
export function yourDevicesQuery(transport: Transport) {
  return {
    ...createQueryOptions(UserService.method.listDevices, {}, { transport }),
    staleTime: "static" as const,
  };
}

function Devices({ devices, whose }: { devices: Device[]; whose: Whose }) {
  if (devices.length === 0) return <Note>No Devices are signed in.</Note>;
  return (
    <ul className="flex flex-col">
      {devices.map((device) => (
        <DeviceRow key={device.id} device={device} whose={whose} />
      ))}
    </ul>
  );
}

/** Whose Devices a `ListDevices` or `RevokeDevice` is about: the caller's own without a `userId`. */
interface Whose {
  userId?: bigint;
}

function devicesOf(user?: User): Whose {
  return user === undefined ? {} : { userId: user.id };
}

function Note({ children }: { children: string }) {
  return <p className="text-sm text-muted-foreground">{children}</p>;
}

function DeviceRow({ device, whose }: { device: Device; whose: Whose }) {
  const queryClient = useQueryClient();
  const revoke = useMutation(UserService.method.revokeDevice, {
    // Refused or not, the list may be out of date.
    onSettled: () => queryClient.invalidateQueries({ queryKey: devicesKey }),
  });
  return (
    <li className="flex min-h-14 items-center gap-3 border-b py-2 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{device.name}</p>
        <p className="text-sm text-muted-foreground">
          {/* Each phrase stays whole when the line wraps. */}
          <span className="whitespace-nowrap">Signed in {dayOf(device.createdAt)}</span> ·{" "}
          <span className="whitespace-nowrap">Last seen {dayOf(device.lastSeenAt)}</span>
        </p>
      </div>
      {device.isCurrent ? (
        <span className="rounded-full border px-2.5 py-0.5 text-sm text-muted-foreground">
          This Device
        </span>
      ) : (
        <ConfirmDialog
          action="Revoke"
          name={`Revoke ${device.name}`}
          title={`Revoke ${device.name}?`}
          description="It will be signed out, and has to sign in again before it can be used."
          onConfirm={() => revoke.mutateAsync({ ...whose, deviceId: device.id })}
          refusal={(error) => refusal(error, "revoke this Device", {}).form}
        />
      )}
    </li>
  );
}
