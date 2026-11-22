import { Segmented } from "@/features/settings/components/segmented";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { backgrounds, radios, sizes } from "@/features/settings/lib/subtitle-appearance";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

/** The rows of the User's Subtitle Appearance: how big their text subtitles are, and what's behind them. */
export function AppearanceRows({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <SettingRow wide {...rowText(playbackLabels, "size")}>
        {(control) => (
          <Segmented
            aria-labelledby={control["aria-labelledby"]}
            aria-describedby={control["aria-describedby"]}
            name="size"
            options={radios(sizes)}
            value={values.size}
            onChange={(size) => {
              onEdit({ size });
            }}
          />
        )}
      </SettingRow>
      <SettingRow wide {...rowText(playbackLabels, "background")}>
        {(control) => (
          <Segmented
            aria-labelledby={control["aria-labelledby"]}
            aria-describedby={control["aria-describedby"]}
            name="background"
            options={radios(backgrounds)}
            value={values.background}
            onChange={(background) => {
              onEdit({ background });
            }}
          />
        )}
      </SettingRow>
    </>
  );
}
