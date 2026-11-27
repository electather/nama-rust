import { HardwareSection } from "@/features/admin-playback/components/hardware-section";
import { PlaybackSettingsFields } from "@/features/admin-playback/components/playback-settings-fields";
import { Programs } from "@/features/admin-playback/components/programs";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import type {
  PlaybackField,
  PlaybackValues,
} from "@/features/admin-playback/lib/playback-settings-form";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SubtitleSpaceRow } from "@/features/settings/components/subtitle-space-row";
import type { GetPlaybackStatusResponse } from "@/gen/nama/v1/server_pb";
import type { FormErrors } from "@/shared/lib/forms";

/** The Server Playback pane's groups: what the Server found, then the settings that save as they're changed, the subtitle cache's last. */
export function ServerPlaybackRows({
  status,
  values,
  errors,
  onEdit,
}: {
  status: GetPlaybackStatusResponse;
  values: PlaybackValues;
  errors: FormErrors<PlaybackField>;
  onEdit: (changes: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <SettingGroup setting="programs" title={serverPlaybackLabels.programs.label}>
        <Programs status={status} />
      </SettingGroup>
      <SettingGroup title="Hardware acceleration">
        <HardwareSection status={status} />
      </SettingGroup>
      <SettingGroup title="Remuxes and Transcodes">
        <PlaybackSettingsFields values={values} errors={errors} onEdit={onEdit} />
      </SettingGroup>
      <SettingGroup title="Subtitles">
        <SubtitleSpaceRow
          value={values.subtitleCacheMib}
          onChange={(subtitleCacheMib) => {
            onEdit({ subtitleCacheMib });
          }}
        />
      </SettingGroup>
    </>
  );
}
