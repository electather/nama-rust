import { Segmented } from "@/features/settings/components/segmented";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type PlaybackValues, playbackLabels } from "@/features/settings/lib/playback-values";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

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
  const switchText = (key: "onlyOther" | "sdh") => ({
    ...rowText(playbackLabels, key),
    ...(forced && { hint: FULL_ONLY }),
  });
  const on = forced ? { onlyOther: false, sdh: false } : values;
  return (
    <>
      <SettingRow wide {...rowText(playbackLabels, "which")}>
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
        {...switchText("onlyOther")}
        checked={on.onlyOther}
        disabled={forced}
        onChange={(onlyOther) => {
          onEdit({ onlyOther });
        }}
      />
      <SwitchRow
        {...switchText("sdh")}
        checked={on.sdh}
        disabled={forced}
        onChange={(sdh) => {
          onEdit({ sdh });
        }}
      />
    </>
  );
}
