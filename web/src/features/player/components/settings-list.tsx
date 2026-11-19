import { cn } from "cn";
import { Clock, Info, SlidersHorizontal } from "lucide-react";

import { AutoplayRow } from "@/features/player/components/autoplay-row";
import { SettingsRow } from "@/features/player/components/settings-row";
import type { SettingsView } from "@/features/player/lib/settings-view";
import { speedName } from "@/features/player/lib/speeds";

/** Speed, the Playback Limit if the plan offers any, autoplay for an Episode, then Playback info. */
export function SettingsList({
  rate,
  limit,
  autoplay,
  go,
  className,
}: {
  rate: number;
  /** The Playback Limit it plays at, if it offers any. */
  limit: string | undefined;
  autoplay: boolean;
  go: (view: SettingsView) => void;
  className: string | undefined;
}) {
  return (
    <div className={cn("flex flex-col gap-0.5 p-1", className)}>
      <SettingsRow
        Icon={Clock}
        label="Speed"
        value={speedName(rate)}
        onClick={() => {
          go("speed");
        }}
      />
      {limit !== undefined && (
        <SettingsRow
          Icon={SlidersHorizontal}
          label="Playback Limit"
          value={limit}
          onClick={() => {
            go("limit");
          }}
        />
      )}
      {autoplay && <AutoplayRow />}
      <hr className="mx-2.5 my-1 border-border" />
      <SettingsRow
        Icon={Info}
        label="Playback info"
        onClick={() => {
          go("info");
        }}
      />
    </div>
  );
}
