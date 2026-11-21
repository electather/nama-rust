import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import { hardwareValue, isSaved, notFoundLabel } from "@/features/admin-playback/lib/hardware";
import { HardwareAcceleration, type PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { type Hardware, hardwareLabel } from "@/shared/lib/playback-names";

/**
 * The choice of what Transcodes run on: Automatic, Off, or an API on a GPU that passed its test,
 * showing the `saved` one.
 */
export function HardwareField({
  saved,
  error,
}: {
  saved: PlaybackSettings;
  error: string | undefined;
}) {
  const { hardware } = usePlaybackStatus();
  const choices: Hardware[] = [
    { api: HardwareAcceleration.UNSPECIFIED },
    { api: HardwareAcceleration.OFF },
    ...hardware,
  ];
  const chosen = choices.find((choice) => isSaved(choice, saved));
  const asSaved = { api: saved.hardwareAcceleration, gpu: saved.gpu };
  return (
    <FormSelect
      label="Hardware acceleration"
      name="hardwareAcceleration"
      defaultValue={hardwareValue(chosen ?? asSaved)}
      hint="What every Transcode decodes, scales and encodes video on. Automatic picks the one marked above. Off runs them in software, and so does a pick that was missing or didn’t pass its test when the Server started. A change applies from the next Transcode."
      error={error}
    >
      {choices.map((choice) => (
        <option key={hardwareValue(choice)} value={hardwareValue(choice)}>
          {hardwareLabel(choice)}
        </option>
      ))}
      {/* Offered as it was saved, so saving doesn't change it. */}
      {chosen === undefined && (
        <option value={hardwareValue(asSaved)}>{notFoundLabel(asSaved)}</option>
      )}
    </FormSelect>
  );
}
