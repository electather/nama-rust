import { Tabs, TabsPanel } from "nama-ui/components/tabs";

import { SeasonTabs } from "@/features/player/components/season-tabs";
import { VirtualEpisodes } from "@/features/player/components/virtual-episodes";
import { useSeasons } from "@/features/player/data/use-seasons";
import { useShowEpisodes } from "@/features/player/data/use-show-episodes";
import { useEpisodeScroll } from "@/features/player/hooks/use-episode-scroll";
import { episodeItems, playsNow } from "@/features/player/lib/episode-items";
import type { InShow } from "@/features/player/lib/in-show";

/**
 * Every Episode of the Show in one list, starting on the one playing, with a tab for each Season
 * when the Show has more than one, which scrolls to its start. `onPick` runs as one is picked.
 */
export function EpisodeList(props: { inShow: InShow; played: number; onPick: () => void }) {
  const { season } = props.inShow;
  const seasons = useSeasons(season.libraryId, season.showId).map(({ number }) => number);
  const episodes = useShowEpisodes(season.libraryId, season.showId, seasons);
  const items = episodeItems(seasons, episodes, seasons.length > 1);
  const scroll = useEpisodeScroll(
    items,
    items.findIndex((item) => playsNow(item, props.inShow)),
  );
  if (seasons.length < 2) {
    return <VirtualEpisodes items={items} scroll={scroll} {...props} />;
  }
  return (
    <Tabs
      value={scroll.shown}
      onValueChange={(value: number) => {
        scroll.show(value);
      }}
    >
      <SeasonTabs seasons={seasons} shown={scroll.shown} />
      <TabsPanel value={scroll.shown}>
        <VirtualEpisodes items={items} scroll={scroll} {...props} />
      </TabsPanel>
    </Tabs>
  );
}
