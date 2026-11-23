import { Plus } from "lucide-react";
import { Button } from "nama-ui/components/button";
import type { RefObject } from "react";

import { RevokeOthers } from "@/features/settings/components/revoke-others";
import type { Device } from "@/gen/nama/v1/sign_in_pb";

/**
 * What acts on the User's `others` Devices: revoking them all, when there's more than one, and
 * signing in another, unless that's already open (`adding`). `addButton` holds that button while
 * it shows, so focus can go back to it.
 */
export function OtherActions({
  others,
  adding,
  addButton,
  onAdd,
}: {
  others: Device[];
  adding: boolean;
  addButton: RefObject<HTMLButtonElement | null>;
  onAdd: () => void;
}) {
  return (
    <div className="flex items-center gap-1">
      {others.length > 1 && <RevokeOthers others={others} />}
      {!adding && (
        // Quiet text beside the group's heading, as tall as it, with a phone's tap area kept around it.
        <Button ref={addButton} variant="link" size="link" onClick={onAdd}>
          <Plus aria-hidden strokeWidth={1.5} className="size-3.5" />
          Sign in a Device
        </Button>
      )}
    </div>
  );
}
