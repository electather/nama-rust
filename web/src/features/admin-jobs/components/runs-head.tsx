import { TOOK_COLUMN } from "@/features/admin-jobs/lib/run-columns";

/** The names of a Run row's columns, over the rows; left out on a phone, where the rows wrap. */
export function RunsHead() {
  return (
    <div
      aria-hidden
      className="flex gap-3 border-b px-2 pt-2 pb-1.5 text-xs text-muted-foreground max-md:hidden"
    >
      <span className="flex-1 ps-7">Started</span>
      <span className={TOOK_COLUMN}>Took</span>
      <span className="w-4" />
    </div>
  );
}
