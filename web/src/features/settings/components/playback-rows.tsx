import { AppearanceRows } from "@/features/settings/components/appearance-rows";
import { AudioRow } from "@/features/settings/components/audio-row";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SubtitleRows } from "@/features/settings/components/subtitle-rows";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { rowText } from "@/shared/lib/setting-text";

/** The Playback pane's rows, under what each group of them is about. */
export function PlaybackRows({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <SettingGroup title="Audio">
        <AudioRow values={values} onEdit={onEdit} />
      </SettingGroup>
      <SettingGroup title="Subtitles">
        <SubtitleRows values={values} onEdit={onEdit} />
        <AppearanceRows values={values} onEdit={onEdit} />
      </SettingGroup>
      <SettingGroup title="Episodes">
        <SwitchRow
          {...rowText(playbackLabels, "autoplay")}
          checked={values.autoplay}
          onChange={(autoplay) => {
            onEdit({ autoplay });
          }}
        />
      </SettingGroup>
    </>
  );
}
