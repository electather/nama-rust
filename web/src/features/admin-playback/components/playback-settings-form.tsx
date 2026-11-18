import { HardwareField } from "@/features/admin-playback/components/hardware-field";
import { RemoteLimitField } from "@/features/admin-playback/components/remote-limit-field";
import { SegmentFields } from "@/features/admin-playback/components/segment-fields";
import { usePlaybackSettings } from "@/features/admin-playback/data/use-playback-settings";
import { useUpdatePlaybackSettings } from "@/features/admin-playback/data/use-update-playback-settings";
import {
  playbackSettingsErrors,
  typedSettings,
} from "@/features/admin-playback/lib/playback-settings-form";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { onSubmitted } from "@/shared/lib/forms";

/**
 * The form an Admin sets how the Server plays Versions it changes with: the folder Remuxes and
 * Transcodes write their segment files to, how much of them they keep for skipping back, what
 * Transcodes run on, the most it runs at once, and the default Playback Limit for remote playback.
 * One form, as saving replaces every Playback setting.
 */
export function PlaybackSettingsForm() {
  const settings = usePlaybackSettings();
  const update = useUpdatePlaybackSettings();
  const errors = playbackSettingsErrors(update.error);

  return (
    <form
      className="mt-10 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: typedSettings(typed) });
      })}
    >
      <SegmentFields saved={settings} errors={errors} />
      <HardwareField saved={settings} error={errors.hardwareAcceleration} />
      <FormField
        label="Transcodes at once"
        name="maxTranscodes"
        type="number"
        min={1}
        required={false}
        defaultValue={settings.maxTranscodes}
        hint="The most the Server runs at once. Remuxes don’t count. While that many run, a Device that can play a Remux gets one instead, and one that can’t is told the Server is busy. Leave it empty for at most 2 at once of those that run on the CPU, in software or tone-mapping HDR there, and no limit for those that run fully on a GPU."
        error={errors.maxTranscodes}
      />
      <RemoteLimitField saved={settings.remotePlaybackLimit} />
      <FormEnd
        action={update.isPending ? "Saving playback settings…" : "Save playback settings"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
