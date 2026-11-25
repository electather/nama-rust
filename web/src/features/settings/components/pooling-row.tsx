import { SwitchRow } from "@/features/settings/components/switch-row";
import { usePooling } from "@/features/settings/data/use-pooling";
import { useSetPooling } from "@/features/settings/data/use-set-pooling";
import { accountHistoryLabels } from "@/features/settings/lib/pane-labels";
import { rowText } from "@/shared/lib/setting-text";

/** Whether the Account's finished Movies and Watchlists count in the pooled counts (ADR 0093). */
export function PoolingRow() {
  const pooling = usePooling();
  const setPooling = useSetPooling();
  return (
    // Last in its own box, so the row gives back its bottom padding before the plays.
    <div>
      <SwitchRow
        {...rowText(accountHistoryLabels, "pooling")}
        error={setPooling.error ? "The Hub couldn’t change this just now. Try again." : undefined}
        checked={pooling}
        disabled={setPooling.isPending}
        onChange={(on) => {
          setPooling.mutate({ pooling: on });
        }}
      />
    </div>
  );
}
