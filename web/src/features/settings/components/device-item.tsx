import { type LucideIcon, Monitor, Smartphone, Tv } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useRef } from "react";

import { ItemRow } from "@/features/settings/components/item-row";
import { lastUsed } from "@/features/settings/lib/last-used";
import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { IconTile } from "@/shared/components/icon-tile";
import { useRevokeDevice } from "@/shared/hooks/use-revoke-device";
import { refusal } from "@/shared/lib/forms";
import type { Opener } from "@/shared/lib/opener";

// ponytail: a Device says only its name, so its kind is guessed from that. Read a kind from the
// Server if it ever sends one.
const KINDS: [pattern: RegExp, Icon: LucideIcon][] = [
  [/\btv\b|tvos|apple tv|android tv|roku|fire tv/iu, Tv],
  [/iphone|ipad|ios|android|phone/iu, Smartphone],
];

function iconOf(name: string) {
  return KINDS.find(([pattern]) => pattern.test(name))?.[1] ?? Monitor;
}

/** One of your Devices: what kind it is, when it was last used, and, unless it's this one, Revoke. */
export function DeviceItem({ device }: { device: Device }) {
  const revoke = useRevokeDevice();
  const asking = useRef<Opener>(null);
  const confirm = (
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
  );
  return (
    <ItemRow
      beside
      start={<IconTile Icon={iconOf(device.name)} />}
      name={device.name}
      meta={lastUsed(device.lastSeenAt)}
    >
      {!device.isCurrent && (
        <Button
          variant="ghost"
          aria-label={`Revoke ${device.name}`}
          aria-haspopup="dialog"
          onClick={() => asking.current?.open()}
        >
          Revoke
        </Button>
      )}
      {!device.isCurrent && confirm}
    </ItemRow>
  );
}
