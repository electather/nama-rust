import type { ComponentProps } from "react";

import type { PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import type { FormErrors } from "@/shared/lib/forms";

/**
 * The fields for the files a Remux or Transcode writes: the folder they go in and how much of them
 * is kept for skipping back, showing the `saved` ones.
 */
export function SegmentFields({
  saved,
  errors,
  saveOn,
}: {
  saved: PlaybackSettings;
  errors: FormErrors;
  /** The events that save a field. */
  saveOn: Pick<ComponentProps<"input">, "onBlur" | "onKeyDown">;
}) {
  return (
    <>
      <FormField
        row
        label="Segment folder"
        name="segmentFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/dev/shm/nama"
        required={false}
        defaultValue={saved.segmentFolder}
        hint="Where a Remux or Transcode writes the files it plays from, such as a tmpfs. Files further behind than Keep for skipping back are deleted as it plays, and the rest a minute after it’s last played. Leave it empty for the transcodes folder in the Server’s data folder. A change applies from the next one."
        error={errors.segmentFolder}
        {...saveOn}
      />
      <FormField
        row
        label="Keep for skipping back"
        name="keepBehindMinutes"
        type="number"
        min={1}
        required={false}
        defaultValue={saved.keepBehindMinutes}
        hint="How much of a Remux or Transcode is kept behind where the player is, so skipping back that far plays at once. Skipping further waits while it’s made again. The kept files take room in the segment folder, memory on a tmpfs, so it must fit them: a 4K Remux keeps about 1 GB for 5 minutes. Leave it empty for 5 minutes. A change applies to those already playing."
        error={errors.keepBehindMinutes}
        {...saveOn}
      />
    </>
  );
}
