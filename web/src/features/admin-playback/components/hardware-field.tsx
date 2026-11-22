import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import {
  hardwareChoices,
  hardwareValue,
  isSaved,
  notFoundLabel,
  savedHardware,
} from "@/features/admin-playback/lib/hardware";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import type { PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { hardwareLabel } from "@/shared/lib/playback-names";
import { rowText } from "@/shared/lib/setting-text";

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
      {...rowText(serverPlaybackLabels, "hardwareAcceleration")}
      name="hardwareAcceleration"
      value={value}
      error={error}
      onValueChange={onPick}
      options={[
        ...choices.map((choice) => ({
          value: hardwareValue(choice),
          label: hardwareLabel(choice),
        })),
        // Offered as it was saved, so saving another setting doesn't change it.
        ...(listed ? [] : [{ value: hardwareValue(asSaved), label: notFoundLabel(asSaved) }]),
      ]}
    />
  );
}
