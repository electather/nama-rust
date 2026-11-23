import { SkeletonRow } from "@/features/settings/components/skeleton-row";

/** Where `rows` of a pane will be while they load. */
export function PaneSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <output className="flex flex-col">
      {Array.from({ length: rows }, (_, row) => (
        <SkeletonRow key={row} />
      ))}
      <span className="sr-only">Loading…</span>
    </output>
  );
}
