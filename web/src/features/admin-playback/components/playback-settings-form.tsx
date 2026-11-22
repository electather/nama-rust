import { HardwareField } from "@/features/admin-playback/components/hardware-field";
import { RemoteLimitField } from "@/features/admin-playback/components/remote-limit-field";
import { SegmentFields } from "@/features/admin-playback/components/segment-fields";
import { usePlaybackSettings } from "@/features/admin-playback/data/use-playback-settings";
import { useSavePlayback } from "@/features/admin-playback/hooks/use-save-playback";
import { playbackSettingsErrors } from "@/features/admin-playback/lib/playback-settings-form";
import { AlertText } from "@/shared/components/form-field/alert-text";
import { FormField } from "@/shared/components/form-field/form-field";

/**
 * The form an Admin sets how the Server plays Versions it changes with: the folder Remuxes and
 * Transcodes write their segment files to, how much of them they keep for skipping back, what
 * Transcodes run on, the most it runs at once, and the default Playback Limit for remote playback.
 * Each change is saved as it's made.
 */
export function PlaybackSettingsForm() {
  const settings = usePlaybackSettings();
  const { error, onPick, field } = useSavePlayback();
  const errors = playbackSettingsErrors(error);

  return (
    <form className="flex flex-col">
      <SegmentFields saved={settings} errors={errors} saveOn={field} />
      <HardwareField saved={settings} error={errors.hardwareAcceleration} onChange={onPick} />
      <FormField
        row
        label="Transcodes at once"
        name="maxTranscodes"
        type="number"
        min={1}
        required={false}
        defaultValue={settings.maxTranscodes}
        hint="The most the Server runs at once. Remuxes don’t count. While that many run, a Device that can play a Remux gets one instead, and one that can’t is told the Server is busy. Leave it empty for at most 2 at once of those that run on the CPU, in software or tone-mapping HDR there, and no limit for those that run fully on a GPU."
        error={errors.maxTranscodes}
        {...field}
      />
      <RemoteLimitField saved={settings.remotePlaybackLimit} onChange={onPick} />
      <AlertText error={errors.form} />
    </form>
  );
}
