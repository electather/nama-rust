import { LanguageSelect } from "@/features/settings/components/language-select";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { SettingRow } from "@/shared/components/setting-row";

/** The row of the User's own Audio Preference: the language audio starts in. */
export function AudioRow({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  return (
    <SettingRow
      label={playbackLabels.audio}
      hint="Audio starts in this language when the file has it."
      size="field"
    >
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
