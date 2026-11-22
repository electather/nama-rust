import { Plus } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { RevokeOthers } from "@/features/settings/components/revoke-others";
import type { Device } from "@/gen/nama/v1/sign_in_pb";

/**
 * What acts on the User's `others` Devices: revoking them all, when there's more than one, and
 * signing in another, unless that's already open (`adding`).
 */
export function OtherActions({
  others,
  adding,
  onAdd,
}: {
  others: Device[];
  adding: boolean;
  onAdd: () => void;
}) {
  return (
    <div className="flex items-center gap-1">
      {others.length > 1 && <RevokeOthers others={others} />}
      {!adding && (
        <Button variant="outline" onClick={onAdd}>
          <Plus aria-hidden strokeWidth={1.5} />
          Sign in a Device
        </Button>
      )}
    </div>
  );
}
