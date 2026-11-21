import { Check, MonitorSmartphone } from "lucide-react";
import { useRef } from "react";

import { IconTile } from "@/features/settings/components/icon-tile";
import { ItemRow } from "@/features/settings/components/item-row";
import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { StateLabel } from "@/shared/components/state-label";
import { useRevokeDevice } from "@/shared/hooks/use-revoke-device";
import { refusal } from "@/shared/lib/forms";
import type { Opener } from "@/shared/lib/opener";
import { dayOf } from "@/shared/lib/title-facts";
import { Button } from "@/shared/ui/button";

/** One of your Devices: when it was last used, and a button to revoke it unless it's this one. */
export function DeviceItem({ device }: { device: Device }) {
  const revoke = useRevokeDevice();
  const asking = useRef<Opener>(null);
  return (
    <ItemRow
      start={<IconTile Icon={MonitorSmartphone} />}
      name={device.name}
      meta={`Last used ${dayOf(device.lastSeenAt)} · Signed in ${dayOf(device.createdAt)}`}
    >
      {device.isCurrent ? (
        <StateLabel tone="positive" label="This Device" Icon={Check} />
      ) : (
        <Button
          variant="secondary"
          aria-label={`Revoke ${device.name}`}
          aria-haspopup="dialog"
          onClick={() => asking.current?.open()}
        >
          Revoke
        </Button>
      )}
      <ConfirmDialog
        ref={asking}
        trigger={false}
        name={`Revoke ${device.name}`}
        action="Revoke Device"
        title={`Revoke ${device.name}?`}
        description="This signs it out right away. It has to sign in again before it can play anything."
        onConfirm={async () => {
          await revoke.mutateAsync({ deviceId: device.id });
        }}
        refusal={(error) => refusal(error, "revoke this Device", {}).form}
      />
    </ItemRow>
  );
}
