import { Check } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { StateLabel } from "@/shared/components/state-label";

/**
 * The foot of the Account's password form: that saving it signs out everywhere else, and its
 * button, which does `action`.
 */
export function AccountPasswordEnd({
  action,
  pending,
  done,
}: {
  action: string;
  pending: boolean;
  done: boolean;
}) {
  return (
    <div className="flex flex-wrap items-center justify-end gap-x-3.5 gap-y-2.5 pt-2.5">
      {/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- resetting the form resets an <output> in it, wiping what React drew there */}
      <span role="status" className="flex items-center gap-2.5 text-sm">
        {done && <StateLabel tone="positive" label="Password saved" Icon={Check} />}
        {done ? "You’re signed out everywhere else." : "You’ll be signed out everywhere else."}
      </span>
      <Button type="submit" disabled={pending}>
        {pending ? "Saving…" : action}
      </Button>
    </div>
  );
}
