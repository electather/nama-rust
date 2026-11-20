import { seasonName } from "@/shared/lib/title-facts";

/** A Season's chip, lit while the row of Episodes is on it, which scrolls the row to its first. */
export function SeasonChip({
  number,
  current,
  onPick,
}: {
  number: number;
  current: boolean;
  onPick: (season: number) => void;
}) {
  return (
    <button
      type="button"
      // Its Season, which the row of chips finds it by to scroll it into view.
      value={number}
      // The row's place, not a page: "true", as a table of contents marks the section in view.
      aria-current={current ? "true" : undefined}
      onClick={() => {
        onPick(number);
      }}
      className="h-8 shrink-0 cursor-pointer snap-start rounded-full bg-secondary px-3.5 text-sm font-medium text-foreground transition-colors outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-current:bg-accent aria-current:font-semibold aria-current:text-primary aria-current:inset-ring aria-current:inset-ring-primary/60"
    >
      {seasonName(number)}
    </button>
  );
}
