import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import type {
  PlaybackField,
  PlaybackValues,
} from "@/features/admin-playback/lib/playback-settings-form";
import { saveOnLeave } from "@/features/admin-playback/lib/save-on-leave";
import { FolderPicker } from "@/shared/components/folder-picker";
import { FormField } from "@/shared/components/form-field/form-field";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";
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
  errors: FormErrors<PlaybackField>;
  onEdit: (changes: Partial<PlaybackValues>) => void;
}) {
  return (
    <>
      <TypedSettingRow
        {...rowText(serverPlaybackLabels, "segmentFolder")}
        saved={values.segmentFolder}
        error={errors.segmentFolder}
        input={{ autoComplete: "off", spellCheck: false, placeholder: "/dev/shm/nama" }}
        renderBeside={(fill) => <FolderPicker path={values.segmentFolder} onPick={fill} />}
        onSave={(segmentFolder) => {
          onEdit({ segmentFolder });
        }}
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
