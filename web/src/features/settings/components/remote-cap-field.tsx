import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { belowOriginal } from "@/shared/lib/playback-names";
import { limitPicked } from "@/shared/lib/remote-limits";

const NO_CAP = PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL;

const options = [
  { value: String(NO_CAP), label: "No cap" },
  ...belowOriginal.map(({ limit, name }) => ({ value: String(limit), label: name })),
];

/** A User's cap for remote playback, `cap`, as a row whose pick goes to `onChange`, to save at once. */
export function RemoteCapField({
  cap,
  onChange,
}: {
  cap: PlaybackLimit;
  onChange: (changes: { cap: PlaybackLimit }) => void;
}) {
  return (
    <FormSelect
      row
      label="Most for remote playback"
      hint="The most their Devices outside the local network are sent, whatever Playback Limit they pick in the player."
      name="remotePlaybackLimit"
      value={String(cap === PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED ? NO_CAP : cap)}
      onValueChange={(value) => {
        onChange({ cap: limitPicked(value) });
      }}
      options={options}
    />
  );
}
