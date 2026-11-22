import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import {
  hardwareChoices,
  hardwareValue,
  isSaved,
  notFoundLabel,
  savedHardware,
} from "@/features/admin-playback/lib/hardware";
import { playbackLabels } from "@/features/admin-playback/lib/playback-settings-form";
import type { PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { hardwareLabel } from "@/shared/lib/playback-names";

/**
 * The choice of what Transcodes run on: Automatic, Off, or an API on a GPU that passed its test,
 * showing the one in `value`, and the `saved` one even if it didn't pass.
 */
export function HardwareField({
  saved,
  value,
  error,
  onPick,
}: {
  saved: PlaybackSettings;
  /** The picked choice's option value. */
  value: string;
  error: string | undefined;
  /** Picking one, which saves it. */
  onPick: (value: string) => void;
}) {
  const choices = hardwareChoices(usePlaybackStatus().hardware);
  const listed = choices.some((choice) => isSaved(choice, saved));
  const asSaved = savedHardware(saved, choices);
  return (
    <FormSelect
      row
      label={playbackLabels.hardwareAcceleration}
      name="hardwareAcceleration"
      value={value}
      hint="What every Transcode decodes, scales and encodes video on. Automatic picks the one marked above. Off runs them in software, and so does a pick that was missing or didn’t pass its test when the Server started. A change applies from the next Transcode."
      error={error}
      onChange={(event) => {
        onPick(event.target.value);
      }}
    >
      {choices.map((choice) => (
        <option key={hardwareValue(choice)} value={hardwareValue(choice)}>
          {hardwareLabel(choice)}
        </option>
      ))}
      {/* Offered as it was saved, so saving another setting doesn't change it. */}
      {!listed && <option value={hardwareValue(asSaved)}>{notFoundLabel(asSaved)}</option>}
    </FormSelect>
  );
}
