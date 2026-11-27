import { type RailStop, railKeyName } from "@/features/library/lib/rail";

/**
 * The quick-scroll rail beside a Library's grid: each letter or year that has titles, which
 * `jump` takes to its first, and a dot where those between two have none.
 */
export function QuickScroll({
  stops,
  titles,
  jump,
}: {
  stops: RailStop[];
  titles: { many: string; byYear: boolean };
  jump: (position: number) => void;
}) {
  return (
    <nav
      aria-label={titles.byYear ? "Jump to a year" : "Jump to a letter"}
      className="flex flex-col"
    >
      {stops.map((stop) =>
        stop.kind === "gap" ? (
          <span
            key={`gap-${stop.after}`}
            aria-hidden
            className="grid h-2.5 place-items-center before:size-0.75 before:rounded-full before:bg-muted-foreground"
          />
        ) : (
          <button
            key={stop.key}
            type="button"
            aria-label={railKeyName(stop.key, titles.many)}
            className="grid h-5 w-full cursor-pointer place-items-center rounded-sm text-xs font-semibold tabular-nums transition-colors outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring sm:h-5.5"
            onClick={() => {
              jump(stop.position);
            }}
          >
            {stop.key}
          </button>
        ),
      )}
    </nav>
  );
}
