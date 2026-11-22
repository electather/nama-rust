import { HardwareField } from "@/features/admin-playback/components/hardware-field";
import { RemoteLimitField } from "@/features/admin-playback/components/remote-limit-field";
import { SegmentFields } from "@/features/admin-playback/components/segment-fields";
import { usePlaybackSettings } from "@/features/admin-playback/data/use-playback-settings";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import type { PlaybackValues } from "@/features/admin-playback/lib/playback-settings-form";
import { saveOnLeave } from "@/features/admin-playback/lib/save-on-leave";
import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";
import { rowText } from "@/shared/lib/setting-text";

/**
 * The fields an Admin sets how the Server plays Versions it changes with: the folder Remuxes and
 * Transcodes write their segment files to, how much of them they keep for skipping back, what
 * Transcodes run on, the most it runs at once, and the default Playback Limit for remote playback.
 * Each shows its one of `values`, and hands `onEdit` its change to save.
 */
export function PlaybackSettingsFields({
  values,
  errors,
  onEdit,
}: {
  values: PlaybackValues;
  /** Why the Server refused the last change, under its field. */
  errors: FormErrors;
  onEdit: (changes: Partial<PlaybackValues>) => void;
}) {
  const saved = usePlaybackSettings();
  return (
    <div className="flex flex-col">
      <SegmentFields values={values} errors={errors} onEdit={onEdit} />
      <HardwareField
        saved={saved}
        value={values.hardwareAcceleration}
        error={errors.hardwareAcceleration}
        onPick={(hardwareAcceleration) => {
          onEdit({ hardwareAcceleration });
        }}
      />
      <FormField
        row
        {...rowText(serverPlaybackLabels, "maxTranscodes")}
        name="maxTranscodes"
        type="number"
        min={1}
        required={false}
        error={errors.maxTranscodes}
        {...saveOnLeave(values.maxTranscodes, (maxTranscodes) => {
          onEdit({ maxTranscodes });
        })}
      />
      <RemoteLimitField
        value={values.remotePlaybackLimit}
        onPick={(remotePlaybackLimit) => {
          onEdit({ remotePlaybackLimit });
        }}
      />
    </div>
  );
}
