import { SpaceSelect } from "@/features/settings/components/space-select";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type ServerValues, serverLabels } from "@/features/settings/lib/server-values";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

/** The rows of how the Server resizes Posters and Backdrops: whether it does, and the disk the copies take. */
export function ImageRows({
  values,
  onEdit,
}: {
  values: ServerValues;
  onEdit: (edits: Partial<ServerValues>) => void;
}) {
  return (
    <>
      <SwitchRow
        {...rowText(serverLabels, "resize")}
        checked={values.resize}
        onChange={(resize) => {
          onEdit({ resize });
        }}
      />
      <SettingRow {...rowText(serverLabels, "space")}>
        {(control) => (
          <SpaceSelect
            control={control}
            value={values.space}
            onChange={(space) => {
              onEdit({ space });
            }}
          />
        )}
      </SettingRow>
    </>
  );
}
