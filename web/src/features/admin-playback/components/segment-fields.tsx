import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import type { PlaybackValues } from "@/features/admin-playback/lib/playback-settings-form";
import { saveOnLeave } from "@/features/admin-playback/lib/save-on-leave";
import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";
import { rowText } from "@/shared/lib/setting-text";

/**
 * The fields for the files a Remux or Transcode writes: the folder they go in and how much of them
 * is kept for skipping back.
 */
export function SegmentFields({
  values,
  errors,
  onEdit,
}: {
  values: PlaybackValues;
  errors: FormErrors;
  onEdit: (changes: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <FormField
        row
        {...rowText(serverPlaybackLabels, "segmentFolder")}
        name="segmentFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/dev/shm/nama"
        required={false}
        error={errors.segmentFolder}
        {...saveOnLeave(values.segmentFolder, (segmentFolder) => {
          onEdit({ segmentFolder });
        })}
      />
      <FormField
        row
        {...rowText(serverPlaybackLabels, "keepBehindMinutes")}
        name="keepBehindMinutes"
        type="number"
        min={1}
        required={false}
        error={errors.keepBehindMinutes}
        {...saveOnLeave(values.keepBehindMinutes, (keepBehindMinutes) => {
          onEdit({ keepBehindMinutes });
        })}
      />
    </>
  );
}
