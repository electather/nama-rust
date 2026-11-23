import { Film, Info, Tv } from "lucide-react";
import { useId } from "react";

import { QualityField } from "@/features/settings/components/quality-field";
import { SelectBox } from "@/features/settings/components/select-box";
import { fetchedName, removedFlag } from "@/features/settings/lib/default-quality";
import { qualitySetting } from "@/features/settings/lib/server-defined";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";
import { IconTile } from "@/shared/components/icon-tile";

/**
 * One Fulfiller's row: what it fetches, a dropdown of its Qualities, its own default marked. It
 * flags a default an Admin has since removed, as the Fulfiller's applies then.
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
  const id = useId();
  const removed = removedFlag(fulfiller);
  const control = {
    id,
    "aria-labelledby": `${id}-label`,
    "aria-describedby": removed === undefined ? `${id}-hint` : `${id}-hint ${id}-removed`,
  };
  const options = fulfiller.qualities.map((offered) => ({
    value: String(offered.id),
    label: offered.isDefault ? `${offered.name} (Default)` : offered.name,
  }));
  const select = (
    <SelectBox control={control} options={options} value={value} onChange={onChange} />
  );
  const flag = removed !== undefined && (
    <p
      id={`${id}-removed`}
      className="flex items-start gap-2 rounded-lg bg-secondary px-2.5 py-2 text-xs text-pretty"
    >
      <Info aria-hidden strokeWidth={1.5} className="mt-px size-3.5 shrink-0" />
      {removed}
    </p>
  );
  return (
    <li data-setting={qualitySetting(fulfiller.pluginId)} className="group flex gap-3">
      <span className="flex h-11 items-center">
        <IconTile Icon={fulfiller.fulfiller === ExtensionPoint.SHOW_FULFILLER ? Tv : Film} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-2 border-t py-1.5 group-first:border-t-0">
        <QualityField id={id} name={fetchedName(fulfiller)} control={select} />
        {flag}
      </div>
    </li>
  );
}
