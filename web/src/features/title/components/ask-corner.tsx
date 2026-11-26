import { Plus } from "lucide-react";

import { askFor, useSeasonAsking } from "@/features/title/hooks/season-asking";
import { cornerClasses } from "@/features/title/lib/corner-classes";
import { seasonName } from "@/shared/lib/title-facts";

/** A missing Episode's +, which Requests its Season `season`, where the page may ask for it. */
export function AskCorner({ season }: { season: number }) {
  const asking = useSeasonAsking();
  return (
    asking && (
      <button
        type="button"
        aria-label={`Request ${seasonName(season)}`}
        disabled={asking.asking.isPending}
        className={`${cornerClasses} transition-colors duration-150 ease-out-expo hover:bg-foreground hover:text-background focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none disabled:opacity-50`}
        onClick={() => {
          askFor(asking, [season]);
        }}
      >
        <Plus aria-hidden className="size-4.5" />
      </button>
    )
  );
}
