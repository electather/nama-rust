import type { Ref } from "react";

import { EpisodeRow } from "@/features/player/components/episode-row";
import { SeasonHeading } from "@/features/player/components/season-heading";
import { type EpisodeItem, playsNow } from "@/features/player/lib/episode-items";
import type { InShow } from "@/features/player/lib/in-show";

/** One row of the Episodes panel's list, at `index`, measured through `ref`. */
export function EpisodeListItem({
  item,
  index,
  ref,
  inShow,
  played,
  onPick,
}: {
  item: EpisodeItem;
  index: number;
  ref: Ref<HTMLLIElement>;
  inShow: InShow;
  played: number;
  onPick: () => void;
}) {
  const { season, episode } = item;
  return (
    <li ref={ref} data-index={index} className="flex snap-start flex-col">
      {episode === undefined ? (
        <SeasonHeading season={season} />
      ) : (
        <EpisodeRow
          episode={episode}
          link={inShow.link(episode)}
          played={playsNow(item, inShow) ? played : undefined}
          onPick={onPick}
        />
      )}
    </li>
  );
}
