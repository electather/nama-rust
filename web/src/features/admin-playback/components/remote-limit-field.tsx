import { limits } from "@/features/admin-playback/lib/remote-limits";
import type { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

/** The field for the default Playback Limit for remote playback, showing the `saved` one. */
export function RemoteLimitField({ saved }: { saved: PlaybackLimit }) {
  return (
    <FormSelect
      label="Default Playback Limit for remote playback"
      name="remotePlaybackLimit"
      defaultValue={saved}
      hint="The most a Device outside the local network is sent until its User picks a Playback Limit in the player. Original sends each Version as it is."
    >
      {limits.map(({ limit, name }) => (
        <option key={limit} value={limit}>
          {name}
        </option>
      ))}
    </FormSelect>
  );
}
