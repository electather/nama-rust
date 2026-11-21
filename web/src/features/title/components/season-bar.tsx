import { SeasonMenu } from "@/features/title/components/season-menu";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * The Season the row of Episodes is on, `current`, over the row: a menu that scrolls it to another
 * of `seasons` when there are more.
 */
export function SeasonBar({
  seasons,
  current,
  onPick,
}: {
  seasons: number[];
  current: number;
  onPick: (season: number) => void;
}) {
  return (
    <h2 className="text-lg font-semibold">
      {seasons.length > 1 ? <SeasonMenu {...{ seasons, current, onPick }} /> : seasonName(current)}
    </h2>
  );
}
