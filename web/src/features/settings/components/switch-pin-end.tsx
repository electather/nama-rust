import { Check } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { StateLabel } from "@/shared/components/state-label";

/** The foot of the Switch PIN form: what the last change `done` was, clearing, and setting. */
export function SwitchPinEnd({
  pending,
  done,
  onClear,
}: {
  pending: boolean;
  done: string | undefined;
  onClear: (form: HTMLFormElement) => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-end gap-x-3.5 gap-y-2.5 pt-2.5">
      {/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- resetting the form resets an <output> in it, wiping what React drew there */}
      <span role="status" className="flex items-center">
        {done !== undefined && <StateLabel tone="positive" label={done} Icon={Check} />}
      </span>
      <Button
        variant="outline"
        disabled={pending}
        onClick={(event) => {
          const { form } = event.currentTarget;
          if (form) {
            onClear(form);
          }
        }}
      >
        Clear Switch PIN
      </Button>
      <Button type="submit" disabled={pending}>
        Set Switch PIN
      </Button>
    </div>
  );
}
