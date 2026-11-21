import { SkeletonRow } from "@/features/settings/components/skeleton-row";

const rows = ["first", "second", "third"];

/** Where a pane's rows will be while they load. */
export function PaneSkeleton() {
  return (
    <output className="flex flex-col">
      {rows.map((row) => (
        <SkeletonRow key={row} />
      ))}
      <span className="sr-only">Loading…</span>
    </output>
  );
}
