import { seasonName } from "@/shared/lib/title-facts";

/** A Season's name, where its Episodes start in the Episodes panel's list. */
export function SeasonHeading({ season }: { season: number }) {
  return (
    <h3 className="px-2.5 pt-3 pb-1.5 text-xs font-medium text-muted-foreground">
      {seasonName(season)}
    </h3>
  );
}
