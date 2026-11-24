import type { Passkey } from "@hub/gen/nama/hub/v1/account_pb";
import { Button } from "nama-ui/components/button";

/** One of the Account's passkeys, by its name, and removing it. */
export function PasskeyRow({
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
    <li className="flex items-center justify-between gap-3 py-1">
      <span className="text-sm font-medium break-all">{passkey.name}</span>
      <Button
        variant="ghost"
        aria-label={`Remove ${passkey.name}`}
        disabled={removing}
        onClick={onRemove}
      >
        Remove
      </Button>
    </li>
  );
}
