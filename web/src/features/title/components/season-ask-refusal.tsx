import { useSeasonAsking } from "@/features/title/hooks/season-asking";
import { seasonsRefusal } from "@/features/title/lib/seasons-refusal";

/** Why asking for Seasons of the page's Show was refused, wherever on the page it was asked. */
export function SeasonAskRefusal() {
  const asking = useSeasonAsking();
  return (
    asking?.asking.isError === true && (
      <p role="alert" className="w-full text-sm text-destructive">
        {seasonsRefusal(asking.asking.error, asking.show.title)}
      </p>
    )
  );
}
