import { KeyCap } from "@/features/action-bar/components/key-cap";

/** What Enter does to the selected row, in a word beside its key. Nothing for a row it can't pick. */
export function RowAction({ action }: { action: string | undefined }) {
  if (action === undefined) {
    return null;
  }
  return (
    <span className="ms-auto inline-flex shrink-0 items-center gap-1.5 text-xs text-foreground">
      <span>{action}</span>
      <KeyCap name="enter" rim />
    </span>
  );
}
