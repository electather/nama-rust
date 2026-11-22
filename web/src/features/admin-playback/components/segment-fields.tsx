import {
  playbackLabels,
  type PlaybackValues,
} from "@/features/admin-playback/lib/playback-settings-form";
import { saveOnLeave } from "@/features/admin-playback/lib/save-on-leave";
import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";

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
        label={playbackLabels.segmentFolder}
        name="segmentFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/dev/shm/nama"
        required={false}
        hint="Where a Remux or Transcode writes the files it plays from, such as a tmpfs. Files further behind than Keep for skipping back are deleted as it plays, and the rest a minute after it’s last played. Leave it empty for the transcodes folder in the Server’s data folder. A change applies from the next one."
        error={errors.segmentFolder}
        {...saveOnLeave(values.segmentFolder, (segmentFolder) => {
          onEdit({ segmentFolder });
        })}
      />
      <FormField
        row
        label={playbackLabels.keepBehindMinutes}
        name="keepBehindMinutes"
        type="number"
        min={1}
        required={false}
        hint="How much of a Remux or Transcode is kept behind where the player is, so skipping back that far plays at once. Skipping further waits while it’s made again. The kept files take room in the segment folder, memory on a tmpfs, so it must fit them: a 4K Remux keeps about 1 GB for 5 minutes. Leave it empty for 5 minutes. A change applies to those already playing."
        error={errors.keepBehindMinutes}
        {...saveOnLeave(values.keepBehindMinutes, (keepBehindMinutes) => {
          onEdit({ keepBehindMinutes });
        })}
      />
    </>
  );
}
