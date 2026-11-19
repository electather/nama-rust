import { LimitChoices } from "@/features/player/components/limit-choices";
import { PlaybackInfo } from "@/features/player/components/playback-info";
import { SpeedChoices } from "@/features/player/components/speed-choices";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Video } from "@/features/player/hooks/use-video";
import type { limitItems } from "@/features/player/lib/limit-items";
import type { SettingsView } from "@/features/player/lib/settings-view";
import type { Track } from "@/gen/nama/v1/browse_pb";

/**
 * The list or controls a settings row opens, or Playback info. A Playback Limit closes the panel,
 * with `close`; the speed keeps it open, to be set finely.
 */
export function SettingsChoices({
  view,
  close,
  video,
  planned,
  tracks,
  limits,
}: {
  view: Exclude<SettingsView, "settings">;
  close: () => void;
  video: Video;
  planned: ReturnType<typeof usePlaybackPlan>;
  tracks: Track[];
  limits: ReturnType<typeof limitItems>;
}) {
  if (view === "info") {
    return <PlaybackInfo planned={planned} tracks={tracks} />;
  }
  if (view === "limit") {
    return (
      <LimitChoices
        {...limits}
        choose={(limit) => {
          video.control((element) => {
            planned.chooseLimit(element, limit);
          });
          close();
        }}
      />
    );
  }
  return (
    <SpeedChoices
      rate={video.state.rate}
      choose={(rate) => {
        video.control((element) => {
          element.playbackRate = rate;
        });
      }}
    />
  );
}
