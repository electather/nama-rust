import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { limits } from "@/shared/lib/remote-limits";
import { rowText } from "@/shared/lib/setting-text";

/** The field for the default Playback Limit for remote playback, showing the one in `value`. */
export function RemoteLimitField({
  value,
  onPick,
}: {
  /** The picked limit's option value. */
  value: string;
  /** Picking one, which saves it. */
  onPick: (value: string) => void;
}) {
  return (
    <FormSelect
      row
      {...rowText(serverPlaybackLabels, "remotePlaybackLimit")}
      name="remotePlaybackLimit"
      value={value}
      onValueChange={onPick}
      options={limits.map(({ limit, name }) => ({ value: String(limit), label: name }))}
    />
  );
}
