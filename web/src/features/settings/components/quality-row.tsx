import { SelectBox } from "@/features/settings/components/select-box";
import { qualityLabel, removedFlag } from "@/features/settings/lib/default-quality";
import { qualitySetting } from "@/features/settings/lib/server-defined";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";
import { SettingRow } from "@/shared/components/setting-row";

/**
 * One Fulfiller's row: a dropdown of its Qualities, its own default marked. It flags a default an
 * Admin has since removed, as the Fulfiller's applies then.
 */
export function QualityRow({
  fulfiller,
  value,
  onChange,
}: {
  fulfiller: DefaultQuality;
  value: string | undefined;
  onChange: (quality: string) => void;
}) {
  const shows = fulfiller.fulfiller === ExtensionPoint.SHOW_FULFILLER;
  const hint =
    removedFlag(fulfiller) ??
    (shows
      ? "If someone else already Requested the Show, new Episodes come in the Quality they picked."
      : undefined);
  return (
    <SettingRow
      setting={qualitySetting(fulfiller.pluginId)}
      label={qualityLabel(fulfiller)}
      hint={hint}
      size="field"
    >
      {(control) => (
        <SelectBox
          control={control}
          options={fulfiller.qualities.map((offered) => ({
            value: String(offered.id),
            label: offered.isDefault ? `${offered.name} (Default)` : offered.name,
          }))}
          value={value}
          onChange={onChange}
        />
      )}
    </SettingRow>
  );
}
