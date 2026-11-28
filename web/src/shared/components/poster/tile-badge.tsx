import type { Shown } from "@/shared/components/state-label";

/** A poster badge's state, with a `short` word to show when its label won't fit a poster's corner. */
export type TileBadgeShown = Shown & { short?: string };

/**
 * Where a missing title stands: a one-line pill in its poster's top corner on the scrim, so it
 * reads over art without hiding it. Its poster's link points at `id` to describe itself to screen
 * readers.
 */
export function TileBadge({ id, badge }: { id: string; badge: TileBadgeShown }) {
  const { label, short = label, Icon } = badge;
  return (
    <span
      id={id}
      className="dark absolute inset-s-2 top-2 inline-flex items-center gap-1 rounded-full bg-scrim px-2 py-0.5 text-xs whitespace-nowrap text-foreground"
    >
      <Icon aria-hidden className="size-3 shrink-0" />
      <span aria-hidden>{short}</span>
      <span className="sr-only">{label}</span>
    </span>
  );
}
