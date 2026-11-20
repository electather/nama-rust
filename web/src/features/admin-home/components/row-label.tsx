import type { RowName } from "@/features/admin-home/lib/home-layout";

/** What a row is called, and the Plugin it's from under it. */
export function RowLabel({ name: { label, from } }: { name: RowName }) {
  return (
    <div className="min-w-0 flex-1">
      <p className="truncate">{label}</p>
      {from !== undefined && <p className="truncate text-sm text-muted-foreground">{from}</p>}
    </div>
  );
}
