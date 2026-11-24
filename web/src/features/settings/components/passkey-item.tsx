import { KeyRound } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { ItemRow } from "@/features/settings/components/item-row";
import type { Passkey } from "@/gen/nama/hub/v1/account_pb";
import { IconTile } from "@/shared/components/icon-tile";

/** One of the Account's passkeys, by its name, and removing it. */
export function PasskeyItem({
  passkey,
  removing,
  onRemove,
}: {
  passkey: Passkey;
  /** Whether a passkey is being removed, this one or another. */
  removing: boolean;
  onRemove: () => void;
}) {
  return (
    <ItemRow beside start={<IconTile Icon={KeyRound} />} name={passkey.name} meta={undefined}>
      <Button
        variant="ghost"
        aria-label={`Remove ${passkey.name}`}
        disabled={removing}
        onClick={onRemove}
      >
        Remove
      </Button>
    </ItemRow>
  );
}
