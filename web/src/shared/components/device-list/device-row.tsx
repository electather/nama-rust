import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { DeviceTimes } from "@/shared/components/device-list/device-times";
import { useRevokeDevice } from "@/shared/hooks/use-revoke-device";
import type { Whose } from "@/shared/lib/devices-of";
import { refusal } from "@/shared/lib/forms";

/** One Device: when it signed in and was last seen, and a button to revoke it unless it's this one. */
export function DeviceRow({ device, whose }: { device: Device; whose: Whose }) {
  const revoke = useRevokeDevice();
  return (
    <li className="flex min-h-14 items-center gap-3 border-b py-2 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{device.name}</p>
        <DeviceTimes device={device} />
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
          onConfirm={async () => {
            await revoke.mutateAsync({ ...whose, deviceId: device.id });
          }}
          refusal={(error) => refusal(error, "revoke this Device", {}).form}
        />
      )}
    </li>
  );
}
