import { cn } from "cn";

import { BackRow } from "@/features/player/components/back-row";
import { SettingsChoices } from "@/features/player/components/settings-choices";
import { SettingsList } from "@/features/player/components/settings-list";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Video } from "@/features/player/hooks/use-video";
import { limitItems } from "@/features/player/lib/limit-items";
import { type SettingsView, type SlideFrom, slideIn } from "@/features/player/lib/settings-view";
import type { Track } from "@/gen/nama/v1/browse_pb";

/**
 * What Playback settings shows at `view`, sliding in `from` a side. A Playback Limit picked closes
 * it, with `close`.
 */
export function SettingsViews({
  view,
  from,
  go,
  close,
  video,
  planned,
  tracks,
  autoplay,
}: {
  view: SettingsView;
  from: SlideFrom | undefined;
  go: (view: SettingsView) => void;
  close: () => void;
  video: Video;
  planned: ReturnType<typeof usePlaybackPlan>;
  tracks: Track[];
  autoplay: boolean;
}) {
  const limits = limitItems(planned.plan, tracks);
  if (view === "settings") {
    return (
      <SettingsList
        rate={video.state.rate}
        limit={limits.items.length > 0 ? limits.playing.name : undefined}
        autoplay={autoplay}
        go={go}
        className={slideIn(from)}
      />
    );
  }
  return (
    <div className={cn("flex min-h-0 flex-col gap-0.5 p-1", slideIn(from))}>
      <BackRow
        title={titles[view]}
        onBack={() => {
          go("settings");
        }}
      />
      {view !== "info" && <hr className="mx-2.5 my-1 border-border" />}
      <SettingsChoices
        view={view}
        close={close}
        video={video}
        planned={planned}
        tracks={tracks}
        limits={limits}
      />
    </div>
  );
}

const titles = { speed: "Speed", limit: "Playback Limit", info: "Playback info" };
