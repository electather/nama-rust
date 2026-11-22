import type { RowName } from "@/shared/lib/home-layout";

/** What a row is called, Muted while it's hidden, and the Plugin it's from under it. */
export function RowLabel({ name: { label, from } }: { name: RowName }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <p className="truncate text-sm transition-colors group-data-hidden:text-muted-foreground motion-reduce:transition-none">
        {label}
      </p>
      {from !== undefined && <p className="truncate text-xs text-muted-foreground">{from}</p>}
    </div>
  );
}
