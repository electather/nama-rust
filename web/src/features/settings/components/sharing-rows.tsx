import { SettingGroup } from "@/features/settings/components/setting-group";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { type ServerValues, serverLabels } from "@/features/settings/lib/server-values";
import { rowText } from "@/shared/lib/setting-text";

/** What the Server shares with Nama's developers: each off until an admin turns it on. */
export function SharingRows({
  values,
  onEdit,
}: {
  values: ServerValues;
  onEdit: (edits: Partial<ServerValues>) => void;
}) {
  return (
    <SettingGroup title="Sharing">
      <SwitchRow
        {...rowText(serverLabels, "shareDiagnostics")}
        checked={values.shareDiagnostics}
        onChange={(shareDiagnostics) => {
          onEdit({ shareDiagnostics });
        }}
      />
      <SwitchRow
        {...rowText(serverLabels, "shareUsageAnalytics")}
        checked={values.shareUsageAnalytics}
        onChange={(shareUsageAnalytics) => {
          onEdit({ shareUsageAnalytics });
        }}
      />
    </SettingGroup>
  );
}
