import { AskedSeason } from "@/features/title/components/asked-season";
import { useSeasonAsking } from "@/features/title/hooks/season-asking";
import { type HeroTarget, targetAsk } from "@/features/title/lib/hero-target";

/**
 * A Show's main button when `target`, the next thing you'd watch, is missing from the Server and
 * the page may ask for its Season.
 */
export function SeasonAsk({ target }: { target: HeroTarget | undefined }) {
  const asking = useSeasonAsking();
  const asked = targetAsk(target);
  return (
    asking && asked && <AskedSeason asking={asking} season={asked.season} request={asked.request} />
  );
}
