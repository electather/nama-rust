import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/lib/forms";
import { devicesKey } from "@/lib/users";

/** A User's Devices, each but this one with a button to revoke it. Your own without a `userId`. */
export function DeviceList({ userId }: { userId?: bigint }) {
  const whose = userId === undefined ? {} : { userId };
  // A failure shows the page's error, as for any other query the page needs.
  const { data } = useQuery(UserService.method.listDevices, whose, { throwOnError: true });
  if (!data) return <Note>Loading…</Note>;
  if (data.devices.length === 0) return <Note>No Devices are signed in.</Note>;
  return (
    <ul className="flex flex-col">
      {data.devices.map((device) => (
        <DeviceRow key={device.id} device={device} whose={whose} />
      ))}
    </ul>
  );
}

function Note({ children }: { children: string }) {
  return <p className="text-sm text-muted-foreground">{children}</p>;
}

function DeviceRow({ device, whose }: { device: Device; whose: { userId?: bigint } }) {
  const queryClient = useQueryClient();
  const revoke = useMutation(UserService.method.revokeDevice, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: devicesKey }),
  });
  return (
    <li className="flex min-h-14 items-center gap-3 border-b py-2 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{device.name}</p>
        <p className="text-sm text-muted-foreground">
          {/* Each phrase stays whole when the line wraps. */}
          <span className="whitespace-nowrap">Signed in {day(device.createdAt)}</span> ·{" "}
          <span className="whitespace-nowrap">Last seen {day(device.lastSeenAt)}</span>
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

/** The day `seconds` since the Unix epoch falls on, as this browser writes dates. */
function day(seconds: bigint) {
  return new Date(Number(seconds) * 1000).toLocaleDateString(undefined, { dateStyle: "medium" });
}
