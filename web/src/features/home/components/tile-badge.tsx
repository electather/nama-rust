import { type Shown, StateLabel } from "@/shared/components/state-label";

/** Where a missing title stands, over its poster's top corner on the scrim, so it reads over art. */
export function TileBadge({ badge }: { badge: Shown }) {
  return (
    <span className="dark absolute inset-s-2 top-2 rounded-full bg-scrim text-foreground">
      <StateLabel {...badge} />
    </span>
  );
}
