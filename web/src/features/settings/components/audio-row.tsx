import { LanguageSelect } from "@/features/settings/components/language-select";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

/** The row of the User's own Audio Preference: the language audio starts in. */
export function AudioRow({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  return (
    <SettingRow {...rowText(playbackLabels, "audio")} size="field">
      {(control) => (
        <LanguageSelect
          control={control}
          none="File’s default"
          value={values.audio}
          onChange={(audio) => {
            onEdit({ audio });
          }}
        />
      )}
    </SettingRow>
  );
}
