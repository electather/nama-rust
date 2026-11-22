import { playbackLabels } from "@/features/admin-playback/lib/playback-settings-form";
import { limits } from "@/features/admin-playback/lib/remote-limits";
import { FormSelect } from "@/shared/components/form-field/form-select";

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
      label={playbackLabels.remotePlaybackLimit}
      name="remotePlaybackLimit"
      value={value}
      onValueChange={onPick}
      hint="The most a Device outside the local network is sent until its User picks a Playback Limit in the player. Original sends each Version as it is."
      options={limits.map(({ limit, name }) => ({ value: String(limit), label: name }))}
    />
  );
}
