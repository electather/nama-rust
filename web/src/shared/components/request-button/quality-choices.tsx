import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { QualityItem } from "@/shared/components/request-button/quality-item";
import { DropdownMenuRadioGroup } from "@/shared/ui/dropdown-menu";

/** `qualities` as radio items, the User's `personalDefault` checked, else the Fulfiller's. */
export function QualityChoices({
  qualities,
  personalDefault,
  onPick,
}: {
  qualities: Quality[];
  personalDefault: bigint | undefined;
  onPick: (quality: bigint) => void;
}) {
  const checked = personalDefault ?? qualities.find(({ isDefault }) => isDefault)?.id;
  return (
    <DropdownMenuRadioGroup value={checked}>
      {qualities.map((quality) => (
        <QualityItem key={quality.id} quality={quality} onPick={onPick} />
      ))}
    </DropdownMenuRadioGroup>
  );
}
