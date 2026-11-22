import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { useRevokeDevice } from "@/shared/hooks/use-revoke-device";
import { refusal } from "@/shared/lib/forms";

/** Signs out every one of the `others`, once asked, leaving only this Device signed in. */
export function RevokeOthers({ others }: { others: Device[] }) {
  const revoke = useRevokeDevice();
  return (
    <ConfirmDialog
      name="Revoke all other Devices"
      action="Revoke all"
      title={`Revoke ${others.length} other Devices?`}
      description="This signs them out right away. Each has to sign in again before it can play anything."
      onConfirm={async () => {
        // ponytail: one call per Device, as the Server revokes one at a time. A Server call that
        // revokes every other Device would make this one step.
        for (const device of others) {
          // oxlint-disable-next-line no-await-in-loop -- one at a time, so a refusal stops the rest
          await revoke.mutateAsync({ deviceId: device.id });
        }
      }}
      refusal={(error) => refusal(error, "revoke these Devices", {}).form}
    />
  );
}
