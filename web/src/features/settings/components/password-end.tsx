import { Check } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { StateLabel } from "@/shared/components/state-label";

/** The foot of the password form: what changing it does to your other Devices, and its button. */
export function PasswordEnd({ pending, done }: { pending: boolean; done: boolean }) {
  return (
    <div className="flex flex-wrap items-center justify-end gap-x-3.5 gap-y-2.5 pt-2.5">
      {/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- resetting the form resets an <output> in it, wiping what React drew there */}
      <span role="status" className="flex items-center gap-2.5 text-sm">
        {done && <StateLabel tone="positive" label="Password changed" Icon={Check} />}
        {done ? "Your other Devices are signed out." : "Your other Devices will be signed out."}
      </span>
      <Button type="submit" disabled={pending}>
        {pending ? "Changing…" : "Change password"}
      </Button>
    </div>
  );
}
