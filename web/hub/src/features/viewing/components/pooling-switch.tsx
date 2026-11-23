import { usePooling } from "@hub/features/viewing/data/pooling";
import { Switch } from "nama-ui/components/switch";
import { useId } from "react";

/**
 * Whether the signed-in Account's finished Movies and Watchlists count in the counts every Account
 * pools, what that means, and that turning it off can't take back the Movies counted (ADR 0093).
 */
export function PoolingSwitch() {
  const pooling = usePooling();
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={id} className="text-sm font-medium">
          Count the Movies you finish
        </label>
        <Switch
          id={id}
          aria-describedby={`${id}-notice`}
          checked={pooling.on}
          disabled={pooling.busy}
          onCheckedChange={(checked) => {
            pooling.set(checked);
          }}
        />
      </div>
      <p id={`${id}-notice`} className="text-xs text-pretty text-muted-foreground">
        Each Movie your Linked Users finish adds one to how many Accounts finished it, and each pair
        of Movies, which recommendations learn from. The counts hold nothing that leads back to you,
        and none is used until 20 Accounts are in it. The titles on your Linked Users’ Watchlists
        count toward Nama’s most watchlisted the same way, only while this is on. Turning this off
        stops new counts, but what’s already counted can’t be taken out.
      </p>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {pooling.failed
          ? "The Hub couldn’t reach this setting just now. Reload the page to try again."
          : undefined}
      </p>
    </div>
  );
}
