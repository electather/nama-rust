import { HardwareSection } from "@/features/admin-playback/components/hardware-section";
import { PlaybackSettingsForm } from "@/features/admin-playback/components/playback-settings-form";
import { Programs } from "@/features/admin-playback/components/programs";
import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SettingGroup } from "@/features/settings/components/setting-group";

/**
 * How the Server plays what it has to change, for an Admin: the ffmpeg and hardware it found to
 * Remux and Transcode with, and its Playback settings, each saved as it's changed.
 */
export function ServerPlaybackPane() {
  const status = usePlaybackStatus();
  return (
    <PaneFrame section="server-playback">
      <SettingGroup title="ffmpeg and ffprobe">
        <Programs status={status} />
      </SettingGroup>
      <SettingGroup title="Hardware acceleration">
        <HardwareSection status={status} />
      </SettingGroup>
      <SettingGroup title="Remuxes and Transcodes">
        <PlaybackSettingsForm />
      </SettingGroup>
    </PaneFrame>
  );
}
