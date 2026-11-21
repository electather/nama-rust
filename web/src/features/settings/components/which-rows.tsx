import { Segmented } from "@/features/settings/components/segmented";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { SettingRow } from "@/shared/components/setting-row";

const FULL_ONLY = "Only applies to Full subtitles.";

/**
 * Which subtitles in the language picked, and when. What forced-only subtitles can't take is
 * switched off, saying why.
 */
export function WhichRows({
  values,
  onEdit,
}: {
  values: PlaybackValues;
  onEdit: (edits: Partial<PlaybackValues>) => void;
}) {
  const forced = values.which === "forced";
  // Neither applies to forced-only subtitles, so each is off and says so while they're picked.
  const hints = forced
    ? { onlyOther: FULL_ONLY, sdh: FULL_ONLY }
    : {
        onlyOther: "Subtitles stay off when the audio is already in your subtitle language.",
        sdh: "Adds sounds like [door slams] when the file has them.",
      };
  const on = forced ? { onlyOther: false, sdh: false } : values;
  return (
    <>
      <SettingRow
        wide
        label={playbackLabels.which}
        hint="Full is every line. Forced only is signs and lines in another language."
      >
        {(control) => (
          <Segmented
            aria-labelledby={control["aria-labelledby"]}
            aria-describedby={control["aria-describedby"]}
            name="which"
            options={[
              ["full", "Full"],
              ["forced", "Forced only"],
            ]}
            value={values.which}
            onChange={(which) => {
              onEdit({ which });
            }}
          />
        )}
      </SettingRow>
      <SwitchRow
        label={playbackLabels.onlyOther}
        hint={hints.onlyOther}
        checked={on.onlyOther}
        disabled={forced}
        onChange={(onlyOther) => {
          onEdit({ onlyOther });
        }}
      />
      <SwitchRow
        label={playbackLabels.sdh}
        hint={hints.sdh}
        checked={on.sdh}
        disabled={forced}
        onChange={(sdh) => {
          onEdit({ sdh });
        }}
      />
    </>
  );
}
