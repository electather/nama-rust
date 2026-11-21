import { ImageRows } from "@/features/settings/components/image-rows";
import { ServerUrlRow } from "@/features/settings/components/server-url-row";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { ServerValues } from "@/features/settings/lib/server-values";

/** The Server pane's rows that save as they're changed, under what each group of them is about. */
export function ServerRows({
  values,
  urlRefused,
  onEdit,
}: {
  values: ServerValues;
  /** Why the Server refused the Server URL, if it did. */
  urlRefused: string | undefined;
  onEdit: (edits: Partial<ServerValues>) => void;
}) {
  return (
    <>
      <SettingGroup title="Address">
        <ServerUrlRow
          saved={values.url}
          refused={urlRefused}
          onSave={(url) => {
            onEdit({ url });
          }}
        />
      </SettingGroup>
      <SettingGroup title="Images">
        <ImageRows values={values} onEdit={onEdit} />
      </SettingGroup>
    </>
  );
}
