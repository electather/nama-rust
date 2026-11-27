import { useSubtitleCacheBytes } from "@/features/admin-playback/data/use-subtitle-cache-bytes";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import { SpaceSelect } from "@/features/settings/components/space-select";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";
import { sizeOf } from "@/shared/lib/title-facts";

/** The row of the most disk the subtitle cache takes, `value` chosen, saying how much it takes now. */
export function SubtitleSpaceRow({
  value,
  onChange,
}: {
  value: string;
  onChange: (subtitleCacheMib: string) => void;
}) {
  const { hint, ...text } = rowText(serverPlaybackLabels, "subtitleCacheMib");
  const used = sizeOf(useSubtitleCacheBytes());
  return (
    <SettingRow {...text} hint={`${hint ?? ""} ${used} in use now.`} size="field">
      {(control) => <SpaceSelect control={control} value={value} onChange={onChange} />}
    </SettingRow>
  );
}
