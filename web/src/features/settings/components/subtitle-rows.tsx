import { LanguageSelect } from "@/features/settings/components/language-select";
import { WhichRows } from "@/features/settings/components/which-rows";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { SettingRow } from "@/shared/components/setting-row";

/**
 * The rows of the User's own Subtitle Preference: the language, and while one is picked, which of
 * its subtitles and when.
 */
export function SubtitleRows({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <SettingRow
        label={playbackLabels.subtitles}
        hint="Pick Off to start with no subtitles."
        size="field"
      >
        {(control) => (
          <LanguageSelect
            control={control}
            none="Off"
            value={values.subtitles}
            onChange={(subtitles) => {
              onEdit({ subtitles });
            }}
          />
        )}
      </SettingRow>
      {values.subtitles !== "" && <WhichRows values={values} onEdit={onEdit} />}
    </>
  );
}
