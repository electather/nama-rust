import { SpaceSelect } from "@/features/settings/components/space-select";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type ServerValues, serverLabels } from "@/features/settings/lib/server-values";
import { SettingRow } from "@/shared/components/setting-row";

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
        label={serverLabels.resize}
        hint="Sends each Poster and Backdrop at the size it’s shown, so a phone isn’t sent a picture made for a TV. Off, every image is sent at full size."
        checked={values.resize}
        onChange={(resize) => {
          onEdit({ resize });
        }}
      />
      <SettingRow
        label={serverLabels.space}
        hint="The most disk the resized copies take. Lowering it removes the oldest; they’re made again when needed."
      >
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
