import type { TileBadgeShown } from "@/shared/components/row-poster/row-tiles";

/**
 * Where a missing title stands: a one-line pill in its poster's top corner on the scrim, so it
 * reads over art without hiding it. Its poster's link names it to screen readers by `id`.
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
